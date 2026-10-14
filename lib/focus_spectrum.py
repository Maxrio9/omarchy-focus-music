"""Five frequency bands from the player's own audio, using a small FFT."""
import cmath
import math

SIZE = 1024
RATE = 22050
BOUNDARIES = (40, 160, 500, 1600, 5000, 10000)
WINDOW = [0.5 - 0.5 * math.cos(2 * math.pi * i / (SIZE - 1)) for i in range(SIZE)]


def spectrum(samples, volume=100):
    values = [complex(samples[i] * WINDOW[i]) for i in range(SIZE)]
    j = 0
    for i in range(1, SIZE):
        bit = SIZE >> 1
        while j & bit:
            j ^= bit
            bit >>= 1
        j ^= bit
        if i < j:
            values[i], values[j] = values[j], values[i]
    length = 2
    while length <= SIZE:
        step = cmath.exp(-2j * math.pi / length)
        for start in range(0, SIZE, length):
            rotation = 1
            for offset in range(length // 2):
                a = values[start + offset]
                b = values[start + offset + length // 2] * rotation
                values[start + offset] = a + b
                values[start + offset + length // 2] = a - b
                rotation *= step
        length *= 2
    # mpv's software volume uses cubic scaling. Recover an approximate
    # pre-volume level so the UI can optionally apply its own volume response.
    scale = max((volume / 100) ** 3, 1e-6)
    bands = []
    for low, high in zip(BOUNDARIES, BOUNDARIES[1:]):
        energy = sum(abs(values[i]) ** 2 for i in range(max(1, math.ceil(low * SIZE / RATE)),
                                                      min(SIZE // 2, math.ceil(high * SIZE / RATE))))
        rms = math.sqrt(energy) * math.sqrt(2 / 0.375) / SIZE / scale
        db = 20 * math.log10(max(rms, 1e-12))
        bands.append(max(0, min(1, (db + 72) / 57)))
    return bands


def player_node(objects, pid):
    clients = {item['id'] for item in objects
               if str(item.get('info', {}).get('props', {}).get('application.process.id')) == str(pid)}
    for item in objects:
        props = item.get('info', {}).get('props', {})
        if (item.get('type', '').endswith(':Node')
                and props.get('media.class') == 'Stream/Output/Audio'
                and (str(props.get('application.process.id')) == str(pid) or props.get('client.id') in clients)):
            return props.get('object.serial', item['id'])
    return None
