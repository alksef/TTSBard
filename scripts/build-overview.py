"""Build both short overviews: python scripts/build-overview.py [--font-dir DIR]."""
import argparse
import json
import os
from pathlib import Path
import shutil

from reportlab.pdfbase import pdfmetrics
from reportlab.pdfbase.ttfonts import TTFont
from reportlab.platypus import BaseDocTemplate, Frame, PageTemplate, Paragraph, Spacer, PageBreak
from pypdf import PdfReader, PdfWriter
from overview_style import (
    A4, mm, PAGE_W, PAGE_H, MARGIN_X, MARGIN_TOP, MARGIN_BOTTOM,
    styles, section, step, decorate_page,
)

ROOT = Path(__file__).resolve().parent.parent


def register_fonts(directory):
    candidates = [Path(directory)] if directory else [
        Path(os.environ.get('WINDIR', 'C:/Windows')) / 'Fonts',
        Path('/usr/share/fonts/truetype/dejavu'),
    ]
    for folder in candidates:
        if (folder / 'segoeui.ttf').is_file():
            files = ['segoeui.ttf', 'segoeuib.ttf', 'segoeuisl.ttf', 'consola.ttf']
        elif (folder / 'DejaVuSans.ttf').is_file():
            files = ['DejaVuSans.ttf', 'DejaVuSans-Bold.ttf', 'DejaVuSans.ttf', 'DejaVuSansMono.ttf']
        else:
            continue
        for name, filename in zip(['SegoeUI', 'SegoeUI-Bold', 'SegoeUI-Semilight', 'Consolas'], files):
            path = folder / filename
            if not path.is_file():
                raise FileNotFoundError(f'Required font is missing: {path}')
            pdfmetrics.registerFont(TTFont(name, str(path)))
        return
    raise FileNotFoundError('Use --font-dir with Segoe UI/Consolas or DejaVu Sans/Mono fonts.')


def build(locale, version, release_dir):
    content = json.loads((ROOT / 'scripts/overview-content' / f'{locale}.json').read_text(encoding='utf-8'))
    output = ROOT / 'docs' / locale / 'ttsbard-overview.pdf'
    output.parent.mkdir(parents=True, exist_ok=True)
    story = [Spacer(1, mm)]
    for block in content['blocks']:
        kind = block['kind']
        if kind == 'paragraph':
            story.append(Paragraph(block['text'].replace('{version}', version), styles[block['style']]))
        elif kind == 'step':
            story.append(step(block['number'], block['text']))
        elif kind == 'section':
            story.extend(section(block['title'], block['lead'], block['bullets']))
        elif kind == 'pagebreak':
            story.append(PageBreak())
        else:
            raise ValueError(f'Unknown content block: {kind}')
    doc = BaseDocTemplate(str(output), pagesize=A4, leftMargin=MARGIN_X,
        rightMargin=MARGIN_X, topMargin=MARGIN_TOP, bottomMargin=MARGIN_BOTTOM,
        title=f'TTSBard {version} - {content["title"]}', author='TTSBard',
        subject=f'TTSBard {version} ({locale})', creator='TTSBard PDF build')
    frame = Frame(MARGIN_X, MARGIN_BOTTOM, PAGE_W - 2*MARGIN_X,
        PAGE_H - MARGIN_TOP - MARGIN_BOTTOM, id='body', leftPadding=0,
        rightPadding=0, topPadding=0, bottomPadding=0)
    doc.addPageTemplates([PageTemplate(id='document', frames=[frame], onPage=decorate_page)])
    doc.build(story)
    reader = PdfReader(output)
    if len(reader.pages) != 2 or version not in reader.pages[0].extract_text():
        raise ValueError(f'{locale}: expected two pages and current version; inspect the content/layout.')
    # Keep links and searchable text while declaring the document language.
    writer = PdfWriter(clone_from=reader)
    from pypdf.generic import NameObject, TextStringObject
    writer.root_object[NameObject('/Lang')] = TextStringObject(locale)
    with output.open('wb') as stream:
        writer.write(stream)
    if release_dir:
        release_dir.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(output, release_dir / f'TTSBard-overview-{version}-{locale}.pdf')
    print(f'{locale}: {output} (2 pages)')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--font-dir', help='Folder containing Segoe UI/Consolas or DejaVu fonts')
    parser.add_argument('--release-dir', type=Path, help='Also copy PDFs here with language/version in filenames')
    args = parser.parse_args()
    register_fonts(args.font_dir)
    version = json.loads((ROOT/'package.json').read_text(encoding='utf-8'))['version']
    for locale in ('en', 'ru'):
        build(locale, version, args.release_dir)


if __name__ == '__main__':
    main()
