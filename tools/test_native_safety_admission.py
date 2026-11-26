"""Regression controls for the frozen #389 native deployment."""
import copy
import unittest

from native_safety_admission import QEMU_SHA256, QEMU_VERSION, deployment_digest, verify_launch


class NativeLaunchTests(unittest.TestCase):
    def setUp(self):
        self.folder = "/relocated/case/pkg"
        self.launch = dict(version=1, qemu_sha256=QEMU_SHA256, qemu_version=QEMU_VERSION,
                           argv=["/usr/local/bin/qemu-system-aarch64", "-machine", "virt,gic-version=2",
                                 "-cpu", "cortex-a72", "-smp", "1", "-accel", "tcg", "-m", "128M",
                                 "-nodefaults", "-display", "none", "-monitor", "none", "-no-reboot",
                                 "-qmp", "stdio", "-serial", "file:/relocated/case/cap/uart.bin",
                                 "-kernel", "/relocated/case/cap/kernel.elf"])

    def test_relocation(self):
        verify_launch(self.launch, self.folder)

    def test_machine_and_paths(self):
        for flag, replacement in (("-cpu","cortex-a53"), ("-machine","virt,gic-version=3"),
                                  ("-smp","2"), ("-accel","kvm"), ("-m","256M"),
                                  ("-kernel","/other/cap/kernel.elf"), ("-serial","file:/other/uart.bin")):
            launch = copy.deepcopy(self.launch)
            launch["argv"][launch["argv"].index(flag)+1] = replacement
            with self.subTest(flag=flag), self.assertRaises(ValueError):
                verify_launch(launch, self.folder)

    def test_clock_device_duplicate_and_missing_options(self):
        for extra in (["-icount","shift=auto"], ["-rtc","clock=vm"], ["-cpu","cortex-a72"],
                      ["-machine","virt"], ["-device","virtio-rng-device"], ["-smp"]):
            launch = copy.deepcopy(self.launch)
            launch["argv"].extend(extra)
            with self.subTest(extra=extra), self.assertRaises(ValueError):
                verify_launch(launch, self.folder)

    def test_emulator_identity(self):
        for key,value in (("qemu_sha256","0"*64), ("qemu_version","other"), ("version",2)):
            with self.subTest(key=key), self.assertRaises(ValueError):
                verify_launch(dict(self.launch, **{key:value}), self.folder)
        self.launch["argv"][0] = "/other/emulator"
        with self.assertRaises(ValueError):
            verify_launch(self.launch, self.folder)

    def test_package_identity_and_relocation(self):
        def package(root):
            paths = [root + "/" + name for name in ("sender","controller","supervisor")]
            return dict(policy="rr",quantum_ticks=625000,
                        package_argv=["builder","--out",root+"/pkg","--elf",paths[0]],
                        slots=[dict(slot=i,input=dict(path=path)) for i,path in enumerate(paths)])
        manifest = package("/first")
        original = deployment_digest(manifest)
        self.assertEqual(original, deployment_digest(package("/second")))
        for key,value in (("policy","fifo"),("quantum_ticks",1),("safety_interference",{})):
            self.assertNotEqual(original, deployment_digest(dict(manifest, **{key:value})))
        manifest["package_argv"].extend(["--quantum-ticks","1"])
        self.assertNotEqual(original, deployment_digest(manifest))


if __name__ == "__main__":
    unittest.main()
