"""Host controls for native display, socket and network package selection."""
import argparse
import unittest

from build_native_el0 import native_service_slots


class Parser(argparse.ArgumentParser):
    def error(self, message):
        raise ValueError(message)


def select(slots, *, display=None, socket=None, net=None, pubsub=None, routes=(),
           mask=None, board="virt", policy="fifo", threads=(), irq=0, children=(), rom=()):
    def option(slot):
        return [] if slot is None else [str(slot)]

    args = argparse.Namespace(peer_elf=list(range(1, slots)), display_service=option(display),
                              socket_service=option(socket), net_service=option(net),
                              pubsub_service=option(pubsub), ipc_route=list(routes),
                              peer3_arg=option(mask), threads=threads, uart_rx_irq=irq,
                              exec_child=children, rom_file=rom)
    return native_service_slots(Parser(), args, policy, board), args.peer3_arg


class NativeServiceSlotTests(unittest.TestCase):
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
