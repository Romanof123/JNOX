"""Host controls for native display, socket and network package selection."""
import argparse
import unittest
from unittest.mock import patch

from build_native_el0 import main, native_service_slots


class Parser(argparse.ArgumentParser):
    def error(self, message):
        raise ValueError(message)


def select(slots, *, display=None, socket=None, net=None, pubsub=None, routes=(),
           mask=None, board="virt", policy="fifo", threads=(), irq=0, children=(), rom=(),
           standby=None, display_standby=None, **ipc):
    def option(slot):
        return [] if slot is None else [str(slot)]

    args = argparse.Namespace(peer_elf=list(range(1, slots)), display_service=option(display),
                              socket_service=option(socket), net_service=option(net),
                              pubsub_service=option(pubsub), ipc_route=list(routes),
                              peer3_arg=option(mask), threads=threads, uart_rx_irq=irq,
                              exec_child=children, rom_file=rom, uart_rx_standby=standby,
                              display_standby=display_standby,
                              mailbox_route=[], notify_route=[], loan_route=[], cap_grant=[],
                              cap_route=[], pipes=0, waitset=0, shared_fd=0)
    vars(args).update(ipc)
    return native_service_slots(Parser(), args, policy, board), args.peer3_arg


class NativeServiceSlotTests(unittest.TestCase):
    def test_shared_fd_requires_a_local_socket_broker(self):
        base = dict(slots=3, socket=2, routes=[(0, 2), (1, 2)], shared_fd=1)
        for policy in ("fifo", "rr"):
            with self.subTest(policy=policy):
                self.assertEqual(select(**base, policy=policy)[0], (None, 2, [0, 1]))
        for change in (dict(socket=None), dict(net=2), dict(routes=[]), dict(board="pi4"),
                       dict(policy="edf"), dict(threads=["2"]), dict(shared_fd=2)):
            with self.subTest(change=change), self.assertRaises(ValueError):
                select(**(base | change))

    def test_net_standby(self):
        base = dict(slots=3, socket=0, net=0, routes=[(1, 0)])
        expected = (None, 0, [1])
        self.assertEqual(select(**base)[0], expected)
        self.assertEqual(select(**base, net_standby=None)[0], expected)
        self.assertEqual(select(**base, net_standby="2")[0], expected)
        for change in (dict(slots=2), dict(net_standby="0"), dict(net=None),
                       dict(socket=None), dict(socket=1), dict(slots=4)):
            with self.subTest(change=change), self.assertRaises(ValueError):
                select(**(base | dict(net_standby="2") | change))

    def test_net_standby_duplicate_cli_option(self):
        argv = ["build_native_el0.py", "--net-standby", "1", "--net-standby", "2"]
        with patch("sys.argv", argv), patch("argparse.ArgumentParser.error", Parser.error):
            with self.assertRaisesRegex(ValueError, "--net-standby may appear only once"):
                main()

    def test_uart_rx_standby(self):
        base = dict(slots=3, display=0, standby="2", irq=1)
        self.assertEqual(select(**base)[0], (0, None, []))
        for change in (dict(slots=2), dict(slots=4), dict(display=None), dict(display=1),
                       dict(standby=None), dict(standby="1"), dict(irq=0), dict(irq=2),
                       dict(board="pi4"), dict(policy="rr"), dict(threads=["2"]),
                       dict(children=["1:child"]), dict(rom=["file=a"]), dict(socket=2),
                       dict(pubsub=3), dict(net=2), dict(pipes=1), dict(waitset=1),
                       dict(routes=[(0, 2)]), dict(loan_route=[(0, 2)]),
                       dict(mailbox_route=[(0, 2)]), dict(notify_route=[(0, 2)]),
                       dict(cap_route=[(0, 2)]), dict(cap_grant=[(0, 2, 1)])):
            with self.subTest(change=change), self.assertRaises(ValueError):
                select(**(base | change))

    def test_uart_rx_irq_display_owner(self):
        for slots, owner in ((1, 0), (2, 0), (2, 1)):
            self.assertEqual(select(slots, display=owner, irq=1)[0], (owner, None, []))
        routes = [(0, 3), (1, 3), (2, 3)]
        self.assertEqual(select(4, display=0, socket=3, routes=routes, irq=1)[0], (0, 3, [0, 1, 2]))
        base = dict(slots=2, display=0, irq=1)
        for change in (dict(display=None), dict(board="pi4"), dict(policy="rr"), dict(irq=2),
                       dict(children=["1:child"]), dict(rom=["file=a"]), dict(threads=["2"])):
            with self.subTest(change=change), self.assertRaises(ValueError):
                select(**(base | change))

    def test_display_standby_profile(self):
        routes = [(0, 3), (1, 3), (2, 3)]
        base = dict(slots=5, display=0, display_standby="2", socket=3, shared_fd=1, routes=routes)
        self.assertEqual(select(**base), ((0, 3, [0, 1, 2]), ["7"]))
        for change in (dict(display_standby=None), dict(display_standby="1"), dict(slots=4),
                       dict(display=1), dict(socket=2), dict(shared_fd=0), dict(board="pi4"),
                       dict(policy="rr"), dict(routes=routes + [(4, 3)]), dict(routes=routes[1:]),
                       dict(irq=1), dict(threads=["2"]), dict(children=["1:child"]), dict(rom=["file=a"]),
                       dict(pipes=1), dict(waitset=1), dict(mailbox_route=[(4, 0)])):
            with self.subTest(change=change), self.assertRaises(ValueError):
                select(**(base | change))

    def test_four_process_local_display(self):
        routes = [(2, 3), (0, 3), (1, 3)]
        for mask in (None, 7):
            self.assertEqual(select(4, display=0, socket=3, routes=routes, mask=mask),
                             ((0, 3, [0, 1, 2]), ["7"]))

    def test_four_process_limits(self):
        routes = [(0, 3), (1, 3), (2, 3)]
        base = dict(slots=4, display=0, socket=3, routes=routes)
        for change in (dict(display=None), dict(display=1), dict(socket=None), dict(socket=0),
                       dict(slots=3), dict(pubsub=3), dict(net=3), dict(policy="rr"),
                       dict(board="pi4"), dict(threads=["2"]), dict(mask=3),
                       dict(routes=routes[1:]), dict(routes=routes[:-1]),
                       dict(routes=routes + [(1, 0)]), dict(routes=routes + [(1, 3)])):
            with self.subTest(change=change), self.assertRaises(ValueError):
                select(**(base | change))

    def test_existing_profiles(self):
        self.assertEqual(select(1, display=0)[0], (0, None, []))
        self.assertEqual(select(2, display=1)[0], (1, None, []))
        self.assertEqual(select(3, socket=2, net=2, routes=[(0, 2), (1, 2)])[0],
                         (None, 2, [0, 1]))
        routes = [(0, 3), (1, 3), (2, 3), (0, 2)]
        self.assertEqual(select(4, socket=2, pubsub=3, routes=routes)[0], (None, 2, [0]))
        for args in (dict(slots=1, display=1), dict(slots=3, display=0),
                     dict(slots=3, socket=2, net=1, routes=[(0, 2)]),
                     dict(slots=3, socket=2), dict(slots=4, socket=2, routes=routes),
                     dict(slots=4, socket=2, net=2, pubsub=3, routes=routes),
                     dict(slots=4, socket=2, pubsub=3, routes=routes + [(3, 2)])):
            with self.subTest(args=args), self.assertRaises(ValueError):
                select(**args)


if __name__ == "__main__":
    unittest.main()
