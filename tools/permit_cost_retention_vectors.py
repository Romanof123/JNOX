"""Linux host child/byte fixture and durable-retention controls; never runs QEMU."""
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

from permit_observer_cost import check
from permit_cost_retention import publish, request_stop, termination_output, verify


class TerminalControls(unittest.TestCase):
    def setUp(self):
        self.stop = dict(reason="guest-terminal", sent=True, escalated=False,
                         signal=15, sender_pid=os.getpid(), sender_name="python3",
                         child_pid=123, target_pid=123, expected_exe="/usr/bin/qemu-system-aarch64",
                         target_exe="/usr/bin/qemu-system-aarch64")
        self.ack = f"qemu-system-aarch64: terminating on signal 15 from pid {os.getpid()} (python3)\n".encode()

    def test_whole_ack_after_split_transport(self):
        data = bytearray()
        for i in range(0,len(self.ack),3):
            data.extend(self.ack[i:i+3])
        self.assertEqual(termination_output(b"",bytes(data),self.stop,0),"matched-sigterm-ack")

    def test_invalid_stream_and_termination(self):
        for data in (self.ack[:-1], self.ack+b"extra\n", b"extra\n"+self.ack,
                     self.ack+self.ack, self.ack.replace(b"signal 15",b"signal 2"),
                     self.ack.replace(b"from pid ",b"from pid 99"),
                     self.ack.replace(b"(python3)",b"(another)")):
            with self.subTest(data=data), self.assertRaises(ValueError):
                termination_output(b"",data,self.stop,0)
        for change in (dict(sent=False),dict(escalated=True),dict(reason="timeout"),
                       dict(reason="containment"),dict(target_pid=888),dict(target_exe="/other")):
            with self.subTest(change=change), self.assertRaises(ValueError):
                termination_output(b"",self.ack,{**self.stop,**change},0)
        for out,rc,stop in ((b"noise",0,self.stop),(b"",1,self.stop),(b"",0,None)):
            with self.assertRaises(ValueError):
                termination_output(out,self.ack,stop,rc)

    @unittest.skipUnless(sys.platform == "linux","requires Linux process identities")
    def test_real_host_child_ack(self):
        child = """import os,signal,sys
from pathlib import Path
def handler(sig,frame):
 p=os.getppid()
 name=Path('/proc/%d/comm'%p).read_text().strip()
 exe=Path(os.readlink('/proc/self/exe')).name
 sys.stderr.write(f'{exe}: terminating on signal {sig} from pid {p} ({name})\\n')
 sys.exit(0)
signal.signal(signal.SIGTERM, handler)
print('CONTROL END',flush=True)
signal.pause()
"""
        with subprocess.Popen([sys.executable,"-c",child],stdout=subprocess.PIPE,stderr=subprocess.PIPE) as proc:
            try:
                self.assertEqual(proc.stdout.readline(),b"CONTROL END\n")
                sent=request_stop(proc,"guest-terminal",os.path.realpath(sys.executable))
                out,err=proc.communicate(timeout=3)
                self.assertEqual(termination_output(out,err,sent,proc.returncode),"matched-sigterm-ack")
            finally:
                if proc.poll() is None:
                    proc.kill()


class RetentionControls(unittest.TestCase):
    def test_missing_or_corrupt_extracted_input_never_matches(self):
        with tempfile.TemporaryDirectory() as tmp:
            p = Path(tmp)/"kernel.elf"
            with self.assertRaises(FileNotFoundError):
                check(p, "0"*64)
            p.write_bytes(b"truncated extracted input")
            with self.assertRaises(ValueError):
                check(p, "0"*64)

    def test_extra_entries_at_each_stage(self):
        for phase in ("source", "copy", "final"):
            with self.subTest(phase=phase), tempfile.TemporaryDirectory() as tmp:
                base = Path(tmp)
                src, dst = base/"src", base/"final"
                src.mkdir()
                (src/"uart").write_bytes(b"actual")
                from shutil import copyfile
                rename = os.rename

                def copy(a, b):
                    copyfile(a, b)
                    if phase == "copy":
                        (Path(b).parent/"unexpected").mkdir()

                def promote(a, b):
                    rename(a, b)
                    if phase == "final":
                        (Path(b)/"unexpected").write_bytes(b"retain")

                if phase == "source":
                    (src/"unexpected").write_bytes(b"retain")
                with patch("permit_cost_retention.shutil.copyfile", side_effect=copy), patch(
                        "permit_cost_retention.os.rename", side_effect=promote), self.assertRaises(ValueError):
                    publish(src, dst, ["uart"], ["uart"])
                self.assertEqual(dst.exists(), phase == "final")
                self.assertEqual(len(list(base.rglob("unexpected"))), 1)

    def test_receipt_must_cover_every_expected_byte_and_name(self):
        with tempfile.TemporaryDirectory() as tmp:
            src, dst = Path(tmp)/"src", Path(tmp)/"copy"
            src.mkdir()
            (src/"uart").write_bytes(b"actual")
            saved = publish(src, dst, ["uart"], ["uart"])
            (dst/"retained-manifest.json").write_text("{}")
            with self.assertRaises(ValueError):
                verify(src, dst, ["uart"], saved)
            (dst/"uart").unlink()
            with self.assertRaises(ValueError):
                verify(src, dst, ["uart"], saved)

    def test_partial_and_failed_copy(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp); src=root/"src"; src.mkdir()
            (src/"uart").write_bytes(b"partial row")
            (src/"stderr").write_bytes(b"raw\x00")
            with self.assertRaises(ValueError):
                publish(src,root/"missing",["uart"],["uart","stderr"])
            with patch("permit_cost_retention.shutil.copyfile",side_effect=lambda a,b: Path(b).write_bytes(b"bad")):
                with self.assertRaises(ValueError):
                    publish(src,root/"corrupt",["uart","stderr"],["uart"])
            self.assertFalse((root/"corrupt").exists())
            result=publish(src,root/"preserved",["uart","stderr"],["uart","stderr"])
            self.assertEqual(result["uart"]["bytes"],11)
            self.assertEqual((root/"preserved"/"stderr").read_bytes(),b"raw\x00")
            with self.assertRaises(ValueError):
                publish(src,root/"preserved",["uart"],["uart"])


if __name__ == "__main__":
    unittest.main()
