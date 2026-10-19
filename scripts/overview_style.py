"""Shared visual styles carried over from the approved short overview."""
from reportlab.lib import colors
from reportlab.lib.enums import TA_LEFT
from reportlab.lib.pagesizes import A4
from reportlab.lib.styles import ParagraphStyle, getSampleStyleSheet
from reportlab.lib.units import mm
from reportlab.platypus import Paragraph

PAGE_W, PAGE_H = A4
MARGIN_X, MARGIN_TOP, MARGIN_BOTTOM = 18 * mm, 14 * mm, 14 * mm

BG = colors.HexColor("#F3F5F8")
PAPER = colors.HexColor("#FFFFFF")
TEXT = colors.HexColor("#20252B")
MUTED = colors.HexColor("#66717E")
ACCENT = colors.HexColor("#416B96")
ACCENT_DARK = colors.HexColor("#2E4F70")
RULE = colors.HexColor("#D3DBE4")
SOFT = colors.HexColor("#EAF0F6")

styles = getSampleStyleSheet()
styles.add(ParagraphStyle(name="DocTitle", fontName="SegoeUI-Bold", fontSize=25,
    leading=29, textColor=TEXT, alignment=TA_LEFT, spaceAfter=2 * mm))
styles.add(ParagraphStyle(name="DocSubtitle", fontName="SegoeUI-Semilight", fontSize=14,
    leading=18, textColor=ACCENT_DARK, spaceAfter=4 * mm))
styles.add(ParagraphStyle(name="Version", fontName="SegoeUI-Bold", fontSize=9,
    leading=11, textColor=ACCENT, spaceAfter=4 * mm))
styles.add(ParagraphStyle(name="Lead", fontName="SegoeUI", fontSize=11,
    leading=14.8, textColor=TEXT, backColor=SOFT, borderColor=RULE,
    borderWidth=0.6, borderPadding=(10, 12, 10, 12), borderRadius=4,
    spaceAfter=4 * mm))
styles.add(ParagraphStyle(name="Section", fontName="SegoeUI-Bold", fontSize=15.5,
    leading=18.5, textColor=ACCENT_DARK, spaceBefore=3 * mm, spaceAfter=1.6 * mm,
    keepWithNext=True))
styles.add(ParagraphStyle(name="SectionLead", fontName="SegoeUI-Bold", fontSize=10.6,
    leading=13.6, textColor=TEXT, spaceAfter=1.6 * mm, keepWithNext=True))
styles.add(ParagraphStyle(name="Body", fontName="SegoeUI", fontSize=10,
    leading=13.5, textColor=TEXT, spaceAfter=1.6 * mm))
styles.add(ParagraphStyle(name="DocBullet", fontName="SegoeUI", fontSize=10,
    leading=13.5, textColor=TEXT, leftIndent=5 * mm, firstLineIndent=-4 * mm,
    bulletIndent=0, bulletFontName="SegoeUI", bulletFontSize=10,
    spaceAfter=0.7 * mm))
styles.add(ParagraphStyle(name="Step", fontName="SegoeUI", fontSize=10,
    leading=13.5, textColor=TEXT, leftIndent=7 * mm, firstLineIndent=-6 * mm,
    bulletIndent=0, bulletFontName="SegoeUI-Bold", bulletFontSize=10,
    bulletColor=ACCENT, spaceAfter=1.5 * mm))
styles.add(ParagraphStyle(name="Link", fontName="SegoeUI", fontSize=10,
    leading=13.5, textColor=TEXT, spaceAfter=1.8 * mm))


def section(title, lead, bullets):
    parts = [Paragraph(title, styles["Section"]), Paragraph(lead, styles["SectionLead"])]
    parts.extend(Paragraph(item, styles["DocBullet"], bulletText="-") for item in bullets)
    for part in parts[:-1]:
        part.keepWithNext = True
    return parts


def step(number, text):
    return Paragraph(text, styles["Step"], bulletText=f"{number}.")


def decorate_page(canvas, doc):
    canvas.saveState()
    canvas.setFillColor(BG)
    canvas.rect(0, 0, PAGE_W, PAGE_H, fill=1, stroke=0)
    canvas.setFillColor(PAPER)
    canvas.rect(10 * mm, 9 * mm, PAGE_W - 20 * mm, PAGE_H - 18 * mm, fill=1, stroke=0)
    canvas.setFillColor(MUTED)
    canvas.setFont("SegoeUI-Semilight", 8)
    canvas.drawRightString(PAGE_W - MARGIN_X, 8.5 * mm, str(doc.page))
    canvas.restoreState()
