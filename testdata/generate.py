#!/usr/bin/env python3
"""Generate test binary files for the binary data viewer."""

import struct
import os
import random

OUT = os.path.dirname(os.path.abspath(__file__))


def write(name, data):
    path = os.path.join(OUT, name)
    with open(path, "wb") as f:
        f.write(data)
    print(f"  {name}: {len(data):,} bytes")


print("Generating test files...")

# 1. Tiny file — edge case: smaller than one row at default stride (256)
write("tiny_16b.bin", bytes(range(16)))

# 2. Empty file — zero bytes
write("empty.bin", b"")

# 3. Single byte
write("one_byte.bin", b"\xAB")

# 4. Exactly 256 bytes — one row at default stride
write("one_row_256.bin", bytes(i & 0xFF for i in range(256)))

# 5. Gradient — smooth ramp, good for verifying stride visually.
#    At stride=256 you should see a clean diagonal gradient.
data = bytes(i & 0xFF for i in range(256 * 512))  # 128 KB
write("gradient_128k.bin", data)

# 6. Striped pattern — alternating 0x00/0xFF rows (256 bytes each).
#    Easy to see stride alignment: stripes should be horizontal when stride=256.
stripe = (b"\x00" * 256 + b"\xFF" * 256) * 256  # 128 KB
write("stripes_128k.bin", stripe)

# 7. Checkerboard — alternating pixels, good for stride-off-by-one detection.
checker = bytes((0xFF if (i // 1 + i // 256) % 2 == 0 else 0x00) for i in range(256 * 256))
write("checker_64k.bin", checker)

# 8. Sync word file — contains known sync patterns embedded in random data.
#    Sync word: 1ACFFC1D
SYNC = bytes([0x1A, 0xCF, 0xFC, 0x1D])
random.seed(42)
rand_data = bytearray(random.getrandbits(8) for _ in range(1_000_000))  # 1 MB

# Plant exact matches at known offsets
exact_offsets = [0, 1000, 50_000, 999_996]
for off in exact_offsets:
    rand_data[off:off+4] = SYNC

# Plant a byte-swapped-16 version: swap pairs → CF1A 1DFC
swap16 = bytes([0xCF, 0x1A, 0x1D, 0xFC])
rand_data[200_000:200_004] = swap16

# Plant a byte-swapped-32 version: reverse 4 bytes → 1DFCCF1A
swap32 = bytes([0x1D, 0xFC, 0xCF, 0x1A])
rand_data[400_000:400_004] = swap32

# Plant a bit-inverted version: ~each byte → E53003E2
inverted = bytes(b ^ 0xFF for b in SYNC)
rand_data[600_000:600_004] = inverted

# Plant a bit-reversed version: reverse bits of each byte
reversed_bits = bytes(int(f"{b:08b}"[::-1], 2) for b in SYNC)
rand_data[800_000:800_004] = reversed_bits

write("sync_1mb.bin", bytes(rand_data))
print(f"    Exact sync 1ACFFC1D at offsets: {exact_offsets}")
print(f"    Byte-swap-16 at 200000, Byte-swap-32 at 400000")
print(f"    Bit-inverted at 600000, Bit-reversed at 800000")

# 9. All zeros — edge case for uniform data
write("zeros_64k.bin", b"\x00" * 65536)

# 10. All 0xFF
write("ones_64k.bin", b"\xFF" * 65536)

# 11. Repeating frame pattern — simulates satellite frame structure.
#    Frame = 4-byte sync + 252 bytes payload, stride=256 should show sync column.
frame_sync = bytes([0x1A, 0xCF, 0xFC, 0x1D])
frame_count = 4000  # ~1 MB
frames = bytearray()
for i in range(frame_count):
    payload = bytes(random.getrandbits(8) for _ in range(252))
    frames.extend(frame_sync + payload)
write("frames_1mb.bin", bytes(frames))
print(f"    {frame_count} frames, each 256 bytes (sync + 252 payload)")
print(f"    At stride=256, sync bytes should form a solid vertical stripe on the left")

# 12. Medium file — 10 MB of pseudo-random data for scroll/performance testing
random.seed(123)
write("random_10mb.bin", bytes(random.getrandbits(8) for _ in range(10_000_000)))

# 13. Off-stride frames — stride=400, to test non-power-of-2 stride values.
frame400 = bytearray()
for i in range(2500):  # 1 MB
    frame400.extend(frame_sync)
    frame400.extend(bytes(random.getrandbits(8) for _ in range(396)))
write("frames_stride400_1mb.bin", bytes(frame400))
print(f"    2500 frames at stride=400, set stride to 400 to see alignment")

# 14. Bit-boundary test — first half all 0x55 (01010101), second half 0xAA (10101010)
half = 32768
write("bit_boundary_64k.bin", b"\x55" * half + b"\xAA" * half)

# 15. Large file with frame sync markers
frame_sync = bytes([0x1A, 0xCF, 0xFC, 0x1D])
frame_count = 700000
frames = bytearray()
for i in range(frame_count):
    payload = bytes(random.getrandbits(8) for _ in range(1496))
    frames.extend(frame_sync + payload)
write("frames_stride1500_1gb.bin", bytes(frames))

print("\nDone! Files in:", OUT)
