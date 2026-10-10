#!/usr/bin/env python3
"""Reproduce and validate the static Telegram custom emoji exports."""
from pathlib import Path
import argparse
import io
import json
import tomllib

import cairosvg
from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parent


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    entries = tomllib.loads((ROOT / 'manifest.toml').read_text())['icon']
    assert len({i['id'] for i in entries}) == len(entries), 'duplicate semantic IDs'
    mapping = json.dumps({i['id']: {'custom_emoji_id': i['custom_emoji_id'], 'emoji': i['emoji']}
                          for i in entries}, ensure_ascii=False, indent=2).encode() + b'\n'
    mapping_path = ROOT / 'generated/mapping.json'
    if args.check:
        assert mapping_path.read_bytes() == mapping, 'stale application icon mapping'
    else:
        mapping_path.write_bytes(mapping)
    preview = Image.new('RGB', (1120, 110 + ((len(entries) + 6) // 7) * 164), '#F2F5F7')
    draw = ImageDraw.Draw(preview)
    font = ImageFont.truetype('DejaVuSans.ttf', 13)
    title = ImageFont.truetype('DejaVuSans.ttf', 25)
    draw.text((28, 20), 'VeloHelp · Петербургское велообщество', font=title, fill='#111111')
    draw.text((28, 58), f'{len(entries)} custom emoji · SVG / PNG / WEBP · 100 × 100', font=font, fill='#137BB2')
    for index, item in enumerate(entries):
        source = ROOT / item['source']
        png = cairosvg.svg2png(url=str(source), output_width=100, output_height=100)
        image = Image.open(io.BytesIO(png)).convert('RGBA')
        webp = io.BytesIO()
        image.save(webp, format='WEBP', lossless=True, method=6)
        for key, data in [('generated', png), ('webp', webp.getvalue())]:
            target = ROOT / item[key]
            if args.check:
                assert target.read_bytes() == data, f'stale export: {target}'
            else:
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(data)
            assert len(data) < 128 * 1024, f'oversized emoji: {target}'
        assert image.size == (100, 100)
        assert image.getpixel((0, 0))[3] == 0, 'opaque exterior'
        assert image.getbbox() is not None, 'empty icon'
        x, y = 20 + index % 7 * 156, 95 + index // 7 * 164
        draw.rounded_rectangle((x, y, x + 145, y + 151), 12, fill='white')
        preview.paste(image, (x + 23, y + 7), image)
        for line, text in enumerate([item['label_ru'], item['id']]):
            label_font = font
            for size in range(13, 7, -1):
                label_font = ImageFont.truetype('DejaVuSans.ttf', size)
                if draw.textbbox((0, 0), text, font=label_font)[2] <= 137:
                    break
            box = draw.textbbox((0, 0), text, font=label_font)
            draw.text((x + (145 - box[2]) / 2, y + 112 + line * 19), text, font=label_font,
                      fill='#111111' if line == 0 else '#137BB2')
    if not args.check:
        preview.save(ROOT / 'generated/preview.png')
        districts = [i for i in entries if i['id'].startswith('area.')]
        if districts:
            sheet = Image.new('RGB', (1440, 100 + ((len(districts) + 5) // 6) * 245), '#F2F5F7')
            sheet_draw = ImageDraw.Draw(sheet)
            sheet_draw.text((28, 20), 'VeloHelp · 18 районов Петербурга', font=title, fill='#111111')
            sheet_draw.text((28, 60), 'Местные символы · чёрный контур / синий акцент', font=font, fill='#137BB2')
            for index, item in enumerate(districts):
                x, y = 12 + index % 6 * 238, 100 + index // 6 * 245
                sheet_draw.rounded_rectangle((x, y, x + 226, y + 233), 15, fill='white')
                png = cairosvg.svg2png(url=str(ROOT / item['source']), output_width=190, output_height=190)
                icon = Image.open(io.BytesIO(png)).convert('RGBA')
                sheet.paste(icon, (x + 18, y + 5), icon)
                text = item['label_ru']
                box = sheet_draw.textbbox((0, 0), text, font=font)
                sheet_draw.text((x + (226 - box[2]) / 2, y + 204), text, font=font, fill='#111111')
            sheet.save(ROOT / 'generated/district-preview.png')
    print(f'{len(entries)} icons: sources, sizes, transparency and reproducible exports OK')


if __name__ == '__main__':
    main()
