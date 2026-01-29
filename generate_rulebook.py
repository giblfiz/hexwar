#!/usr/bin/env python3
"""Generate HEXWAR Copper Pass rulebook PDF."""

import json
from pathlib import Path
from fpdf import FPDF

# Load copper-pass ruleset
with open("board_sets/d7_firstarrow_seeds/copper-pass.json") as f:
    data = json.load(f)
    ruleset = data['ruleset']

# Piece name mappings for this matchup
WHITE_NAMES = {
    'F2': 'Wyvern',
    'D5': 'Warboar',
    'D2': 'Troll',
    'C1': 'Orc',
    'A5': 'Goblin',
    'K2': 'King',
}

BLACK_NAMES = {
    'B3': 'Ghast',
    'D1': 'Nightmare',
    'P1': 'Skeleton',
    'G1': 'Specter',
    'K1': 'Necromancer',
}

class RulebookPDF(FPDF):
    def header(self):
        if self.page_no() > 1:
            self.set_font('Helvetica', 'I', 8)
            self.cell(0, 10, 'HEXWAR: Copper Pass - Orcs & Goblins vs Necromancers', 0, 0, 'C')
            self.ln(10)

    def footer(self):
        self.set_y(-15)
        self.set_font('Helvetica', 'I', 8)
        self.cell(0, 10, f'Page {self.page_no()}', 0, 0, 'C')

    def chapter_title(self, title):
        self.set_font('Helvetica', 'B', 14)
        self.set_fill_color(220, 220, 220)
        self.cell(0, 10, title, 0, 1, 'L', True)
        self.ln(2)

    def section_title(self, title):
        self.set_font('Helvetica', 'B', 11)
        self.cell(0, 8, title, 0, 1, 'L')

    def body_text(self, text):
        self.set_font('Helvetica', '', 10)
        self.multi_cell(0, 5, text)
        self.ln(2)

    def bullet_point(self, text):
        self.set_font('Helvetica', '', 10)
        self.set_x(10)  # Reset to left margin
        self.multi_cell(0, 5, f"   - {text}")


pdf = RulebookPDF()
pdf.set_auto_page_break(auto=True, margin=15)

# Title Page
pdf.add_page()
pdf.set_font('Helvetica', 'B', 28)
pdf.ln(40)
pdf.cell(0, 15, 'HEXWAR', 0, 1, 'C')
pdf.set_font('Helvetica', 'B', 18)
pdf.cell(0, 10, 'Copper Pass', 0, 1, 'C')
pdf.ln(5)
pdf.set_font('Helvetica', 'I', 14)
pdf.cell(0, 10, 'Orcs & Goblins vs Necromancers', 0, 1, 'C')
pdf.ln(20)
pdf.set_font('Helvetica', '', 11)
pdf.multi_cell(0, 6,
    "An asymmetric hex-based strategy game where two unique armies clash. "
    "The brutish Orcs charge forward with Trolls and Wyverns while the "
    "Necromancer's undead horde shambles inexorably, with Skeletons that "
    "refuse to stay dead and an untouchable Specter.", align='C')

# Overview
pdf.add_page()
pdf.chapter_title('Overview')
pdf.body_text(
    "HEXWAR is a turn-based strategy game played on a hexagonal board. If you know chess, "
    "you'll find familiar concepts: pieces move in specific patterns, you capture by moving "
    "onto an enemy, and the goal is to capture the enemy's royal piece (here called the King "
    "or Necromancer).\n\n"
    "Key differences from chess:\n"
    "- Hexagonal board (61 hexes) instead of square\n"
    "- Pieces have FACING - they point in a direction, and most can only move relative to where they're pointing\n"
    "- Each turn you take multiple ACTIONS, not just one move\n"
    "- The two armies are completely different (asymmetric)\n"
    "- 50-turn limit with proximity tiebreaker")

pdf.chapter_title('Victory Conditions')
pdf.body_text(
    "1. KING CAPTURE: Capture the enemy King/Necromancer. Instant win.\n\n"
    "2. TURN LIMIT (Turn 50): If no King is captured by turn 50, the King closer to "
    "the center hex wins. If tied, the player with more pieces wins. If still tied, "
    "White (Orcs) wins.")

# The Board
pdf.chapter_title('The Board')
pdf.body_text(
    "The board is a regular hexagon with 8 hexes per edge (61 total hexes). "
    "The center hex is the most important position for the tiebreaker rule.\n\n"
    "Coordinates use an axial system with center at (0,0). You don't need to know "
    "the math - just know that 'closer to center' matters for the tiebreaker.")

# Facing and Directions
pdf.chapter_title('Facing & Directions')
pdf.body_text(
    "Every piece points in one of six directions: N, NE, SE, S, SW, NW.\n\n"
    "Most pieces can only move in directions RELATIVE to where they're facing:\n"
    "- Forward: straight ahead\n"
    "- Forward-Left / Forward-Right: 60 degrees off forward\n"
    "- Back-Left / Back-Right: 120 degrees off forward\n"
    "- Backward: directly behind\n\n"
    "A piece facing North that can move 'Forward' moves North. The same piece "
    "rotated to face East would move East instead.\n\n"
    "'Forward Arc' means Forward + Forward-Left + Forward-Right (the front 180 degrees).\n"
    "'All Directions' means any of the six directions.")

# Movement Types
pdf.chapter_title('Movement Types')

pdf.section_title('STEP (1, 2, or 3)')
pdf.body_text(
    "Move up to N hexes in a straight line in an allowed direction. "
    "You may stop at any point (1, 2, or 3 hexes). Cannot pass through any piece. "
    "Captures by landing on an enemy.")

pdf.section_title('SLIDE')
pdf.body_text(
    "Move any number of hexes in a straight line until blocked. Like a chess Rook or Bishop, "
    "but on hex directions. Cannot pass through pieces. Captures by landing on an enemy.")

pdf.section_title('JUMP')
pdf.body_text(
    "Leap to a hex exactly N spaces away, ignoring all pieces in between. "
    "Like a chess Knight, but the distance and direction vary by piece. "
    "Captures by landing on an enemy. Cannot land on friendly pieces.")

# Turn Structure
pdf.add_page()
pdf.chapter_title('Turn Structure')
pdf.body_text(
    "Each turn, you take a sequence of ACTIONS. There are two action types:\n\n"
    "MOVE: Move one piece according to its movement rules, OR use a move-based special ability.\n\n"
    "ROTATE: Change one piece's facing to any of the six directions.\n\n"
    "The sequence of actions depends on your army's ACTION TEMPLATE:")

pdf.section_title('Template E (Both armies in Copper Pass)')
pdf.body_text(
    "Move - Rotate - Rotate\n\n"
    "You move one piece, then may rotate up to two pieces (can be the same piece, "
    "different pieces, or include the piece that just moved).\n\n"
    "Any action may be skipped (passed) if you have no good moves.")

# The Armies
pdf.add_page()
pdf.chapter_title('The Armies')

pdf.set_font('Helvetica', 'B', 12)
pdf.set_fill_color(200, 230, 200)
pdf.cell(0, 8, 'ORCS & GOBLINS (White) - The Horde', 0, 1, 'L', True)
pdf.ln(2)

pdf.section_title('King (K2) - 1x')
pdf.body_text("Step-1, Forward Arc only. Your victory piece - protect it!")

pdf.section_title('Wyvern (F2) - 2x')
pdf.body_text("Jump-3, All Directions. Leaps exactly 3 hexes in any direction, ignoring pieces in between. "
              "Your most mobile piece - devastating flankers that can strike from unexpected angles.")

pdf.section_title('Warboar (D5) - 1x')
pdf.body_text("Slide, All Directions. Moves any distance in any of the 6 directions until blocked. "
              "Your most powerful piece - like a chess Queen on hexes.")

pdf.section_title('Troll (D2) - 2x')
pdf.body_text("Slide, Forward and Backward only. Charges in a straight line forward or retreats backward. "
              "Devastating when pointed at the enemy, vulnerable from the sides.")

pdf.section_title('Orc (C1) - 4x')
pdf.body_text("Step-3, Forward only. Charges up to 3 hexes straight ahead. Your main infantry - "
              "rotate them to face the enemy, then charge!")

pdf.section_title('Goblin (A5) - 2x')
pdf.body_text("Step-1, Forward-Left and Forward-Right only. Sneaky flankers that move diagonally. "
              "Weak but useful for harassment and controlling space.")

pdf.ln(5)
pdf.set_font('Helvetica', 'B', 12)
pdf.set_fill_color(200, 200, 230)
pdf.cell(0, 8, 'NECROMANCERS (Black) - The Undead', 0, 1, 'L', True)
pdf.ln(2)

pdf.section_title('Necromancer (K1) - 1x')
pdf.body_text("Step-1, All Directions. More mobile than the Orc King but still vulnerable. Protect at all costs!")

pdf.section_title('Ghast (B3) - 4x')
pdf.body_text("Step-2, All Directions. Fast undead hunters that can move up to 2 hexes in any direction. "
              "Your main strike force - mobile and deadly.")

pdf.section_title('Nightmare (D1) - 2x')
pdf.body_text("Slide, Forward only. Spectral steeds that charge unlimited distance straight ahead. "
              "Point them at a target and let them run. Devastating but directional.")

pdf.section_title('Skeleton (P1) - 2x - SPECIAL: Rebirth')
pdf.body_text("Step-1, Forward Arc. When a Skeleton is captured, it goes to YOUR graveyard. "
              "On a later turn, instead of moving, you may RESURRECT it: place it on any empty hex "
              "adjacent to your Necromancer, facing toward the Necromancer. Skeletons keep coming back!")

pdf.section_title('Specter (G1) - 1x - SPECIAL: Phased')
pdf.body_text("Step-1, All Directions. CANNOT capture and CANNOT BE CAPTURED. "
              "It just blocks movement and occupies space. Use it to control key hexes, "
              "block enemy pieces, or scout safely. Immune to everything except being bumped around.")

# Special Abilities Detail
pdf.add_page()
pdf.chapter_title('Special Abilities')

pdf.section_title('Skeleton Rebirth (Costs your MOVE action)')
pdf.body_text(
    "When your Skeleton is captured, it goes to your graveyard (not removed from game).\n\n"
    "On any later turn, instead of your normal Move action, you may Resurrect:\n"
    "1. Choose a Skeleton from your graveyard\n"
    "2. Place it on any EMPTY hex adjacent to your Necromancer\n"
    "3. It faces toward the Necromancer\n\n"
    "You cannot resurrect if there are no empty hexes next to your Necromancer, "
    "or if your graveyard is empty. Kings can never be resurrected (capturing them ends the game).")

pdf.section_title('Specter Phased (Passive - always active)')
pdf.body_text(
    "The Specter has no offensive capability - it literally cannot capture anything.\n"
    "But it also cannot be captured by any enemy piece.\n\n"
    "It still blocks movement (pieces can't move through or land on it).\n"
    "It can be swapped with by friendly special abilities.\n\n"
    "Use it to: block key lanes, protect your Necromancer, or scout enemy territory safely.")

# Starting Setup
pdf.add_page()
pdf.chapter_title('Starting Setup - Copper Pass')

pdf.body_text(
    "The board is oriented with White (Orcs) starting in the South and Black (Necromancers) "
    "starting in the North.\n\n"
    "White pieces start in rows 2-4 from the south edge.\n"
    "Black pieces start in rows 2-4 from the north edge.\n\n"
    "Initial facings are set so pieces generally face toward the center/enemy.")

pdf.section_title('White (Orcs & Goblins) Starting Positions:')
# Count pieces
white_counts = {}
for p in ruleset['white_pieces']:
    white_counts[p] = white_counts.get(p, 0) + 1
white_counts[ruleset['white_king']] = 1

for pid, count in sorted(white_counts.items()):
    name = WHITE_NAMES.get(pid, pid)
    pdf.body_text(f"  {name} ({pid}): {count}x")

pdf.section_title('Black (Necromancers) Starting Positions:')
black_counts = {}
for p in ruleset['black_pieces']:
    black_counts[p] = black_counts.get(p, 0) + 1
black_counts[ruleset['black_king']] = 1

for pid, count in sorted(black_counts.items()):
    name = BLACK_NAMES.get(pid, pid)
    pdf.body_text(f"  {name} ({pid}): {count}x")

pdf.ln(5)
pdf.body_text(
    "See the included board diagram for exact starting positions and facings. "
    "Pieces are shown with an arrow or wedge indicating their facing direction.")

# Strategy Tips
pdf.add_page()
pdf.chapter_title('Strategy Tips')

pdf.section_title('For Orcs & Goblins:')
pdf.bullet_point("Your Wyverns are your secret weapon - they can jump behind enemy lines")
pdf.bullet_point("Trolls are devastating but vulnerable from the sides - protect their flanks")
pdf.bullet_point("Orcs need to rotate before they can charge - plan your rotations!")
pdf.bullet_point("The Warboar is powerful but losing it hurts - don't overextend")
pdf.bullet_point("Your King can only move forward - keep escape routes open")

pdf.ln(3)
pdf.section_title('For Necromancers:')
pdf.bullet_point("Skeletons are expendable - trade them, they'll come back")
pdf.bullet_point("Your Specter cannot die - use it aggressively to block and harass")
pdf.bullet_point("Ghasts are fast and mobile - use them to hunt isolated pieces")
pdf.bullet_point("Nightmares need clear lanes - don't block your own charges")
pdf.bullet_point("Keep empty hexes near your Necromancer for Skeleton resurrection")

pdf.ln(3)
pdf.section_title('General:')
pdf.bullet_point("Control the center - it matters for the tiebreaker")
pdf.bullet_point("Remember you get Move + Rotate + Rotate each turn")
pdf.bullet_point("Rotating a piece doesn't move it - but sets up next turn's attack")
pdf.bullet_point("Watch for Jump and Slide threats - they can strike from far away")

# Quick Reference
pdf.add_page()
pdf.chapter_title('Quick Reference')

pdf.set_font('Courier', '', 9)
pdf.multi_cell(0, 4, """
ORCS & GOBLINS (White)              NECROMANCERS (Black)
========================            ========================
King (K2)    Step-1 Fwd Arc         Necromancer (K1) Step-1 All
Wyvern (F2)  Jump-3 All [x2]        Ghast (B3)   Step-2 All [x4]
Warboar (D5) Slide All [x1]         Nightmare (D1) Slide Fwd [x2]
Troll (D2)   Slide Fwd/Back [x2]    Skeleton (P1) Step-1 Fwd Arc [x2]
Orc (C1)     Step-3 Fwd [x4]                      *Rebirth ability*
Goblin (A5)  Step-1 Diag-Fwd [x2]   Specter (G1) Step-1 All [x1]
                                                  *Cannot capture/be captured*

TURN STRUCTURE (Template E)
===========================
1. MOVE one piece (or use Skeleton Rebirth)
2. ROTATE any piece (optional)
3. ROTATE any piece (optional)

MOVEMENT KEY
============
Step-N: Move 1 to N hexes, blocked by pieces
Slide:  Move any distance until blocked
Jump-N: Leap exactly N hexes, ignores pieces between

VICTORY
=======
* Capture enemy King/Necromancer = Instant Win
* Turn 50: Closer to center wins, then piece count, then White wins
""")

# Save
output_path = Path("HEXWAR_Copper_Pass_Rules.pdf")
pdf.output(str(output_path))
print(f"Rulebook saved to: {output_path.absolute()}")
