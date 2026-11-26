(
    echo "LD SAMPLE eth0 transport $($bb readlink -f /sys/class/net/eth0/device/..)"
    for resource in /sys/class/net/eth0/device/../resource; do
        [ ! -r "$resource" ] || $bb cat "$resource"
    done
    for delay in 0 1 2 4 8; do
        $bb sleep "$delay"
        echo "LD SAMPLE uptime $($bb cat /proc/uptime)"
        $bb grep -E 'CPU|virtio|eth0|arch_timer' /proc/interrupts || :
        $bb grep eth0 /proc/net/dev || :
        $bb cat /proc/net/arp
    done
) >/dev/console 2>&1 &
