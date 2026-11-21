import { Bot, Boxes, Camera, Clapperboard, Cog, Folder, LayoutDashboard, Map as MapIcon, Mic, Music, Package, Palette, Shirt, Sparkles, Sword, Target, Volume2, type LucideIcon } from "lucide-react";

/** Ícone genérico para categorias do GameBanana que não são uma classe do jogo (essas usam o ícone do próprio TF2). */
const RULES: [RegExp, LucideIcon][] = [
  [/skin|cosmetic|hat|outfit|paint/i, Shirt],
  [/\bmaps?\b/i, MapIcon],
  [/hud|\bgui|interface/i, LayoutDashboard],
  [/animation/i, Clapperboard],
  [/hitsound|killsound|killstreak|crit|domination/i, Target],
  [/music|song|jingle|end of round/i, Music],
  [/voice|announcer|speech|taunt|kick/i, Mic],
  [/freeze|\bcam/i, Camera],
  [/sound|audio/i, Volume2],
  [/weapon|melee|items?\b/i, Sword],
  [/model/i, Boxes],
  [/texture|decal|spray|material/i, Palette],
  [/effect|explosion|particle|flash|fire/i, Sparkles],
  [/pack|collection|bundle/i, Package],
  [/script|config|tool|game files|prefab|serverside/i, Cog],
  [/robot|bot|npc|mvm|mann/i, Bot],
];

export function CatGlyph({ name }: { name: string }) {
  const C = RULES.find(([re]) => re.test(name))?.[1] ?? Folder;
  return <C className="cat-glyph" aria-hidden="true" strokeWidth={2.2} absoluteStrokeWidth />;
}
