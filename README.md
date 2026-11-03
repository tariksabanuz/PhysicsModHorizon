# Horizon Physics

A native mod for [Horizon](https://github.com/horizon-rust/horizon) (Minecraft Java 26.3 in Rust) that
brings PhysicsMod-style physics to the client, written from scratch:

- **Quick setup**: the first time the game starts with the mod, the title screen shows a quick setup:
  a page each for mobs, blocks, weather and collapsing structures, each choice a live preview of
  itself (a pig dying, a stone block breaking, rain on a pond, a cliff coming down) made by the
  mod's own solver. Pick one (or Off) per page, or Skip Setup. It can be run again from the mod's
  settings.
- **Crumbling blocks**: a broken block falls apart into pieces of its own texture that tumble, bounce,
  pile up and fade after a while, in one of four styles: blocky (whole), particles (small cubes),
  crumbled voxel (lumps) or fractured (shards).
- **Collapsing**: what a broken block held up and that no longer reaches the ground falls: cut a
  tree's trunk and it tips over away from you, whole logs tumbling down with its leaves; take a
  bridge's pillar and the bridge comes down. The blocks leave the world as if broken (their drops
  fall, in survival). Only blocks the player breaks, in a world played on this computer.
- **Mob ragdolls**: a mob that dies drops as a ragdoll: its model parts (head, body, arms, legs) stay
  joined where they meet and fall limply.
- **Mobs falling apart**: or the parts come loose and tumble separately (blocky), or break up into
  pieces of their own skin (fractured).
- **Weather**: rain splashes up off the ground where it lands.
- **Shoving**: walk into a ragdoll or a pile of pieces and you push them along; they tumble away.
- **Item physics**: dropped items tumble while they fly and lie flat on the ground instead of floating
  and spinning.
- **Explosions**: TNT, creepers, beds and the rest blow their blocks apart: the pieces are thrown out
  of the blast, with an orange fireball, sparks that streak and bounce and a column of dark smoke.
  Pieces lying around and mobs killed by the blast are thrown too.
- **Smoke and fire**: fire, soul fire, lit campfires and open lava give off soft, see-through smoke
  that rises, swirls, drifts with the wind and gathers and spreads under ceilings, and glowing embers.
- **Snow**: snow layers give way: where the player and the mobs walk, the snow is pressed down an
  eighth of a block (as far as feet sink in it) into a soft trough with a little lip of pushed-up
  snow beside it, real terrain shaded like the game shades its faces, and each step kicks up powder.
  The prints stay while the blocks are loaded (up to 3000 blocks).
- **Ropes**: leads (round, lit, heavy: the wind barely moves them) and fishing lines sag, swing and
  lie on the blocks.
- **Cloth capes**: capes are cloth that the air drags: they trail and lift when you run, blow out in
  the wind and hang down when you stand.
- **Water**: how much water moves depends on how deep and how open it is: deep open water rolls in a
  real swell, a block and more from trough to crest with sharp crests, deep blue-green with clear teal
  where the sun shines through; rivers and shallow lakes lap; the shoreline, puddles and water you
  place stay still. Boats ride the swell: they rise, fall and tilt with the water under them (the one
  you sit in only tilts). All of it rougher in wind, rain and thunder, lit as water: the
  sky mirrored at a glancing angle, the sun and moon glittering on the ripples, froth on the crests.
- **Swaying plants**: leaves rustle, grass, flowers and crops bend in gusts that sweep across the
  fields, vines swing, seagrass and kelp move in the current. Lava heaves slowly.
- **Liquids (beta)**: water or lava you pour splashes out as droplets that run over the blocks before
  they settle; falling into water splashes, with froth.
- **Wind**: calm, a breeze or strong, for smoke, capes, ropes and the swaying.
- **Gravity**: Earth, Moon, Mars, Pluto or none, for the pieces.

Settings: Options > Mods > Horizon Physics (saved in `config/horizon_physics.txt`).

How the snow is pressed: each pressed block keeps a height map (8 by 8 cells); its section is
meshed again (at most ten times a second), and the mod swaps the block's flat top for a grid at
those heights.

How the water knows where it is: when Horizon meshes a chunk, the mod raises each still water
surface corner by a mark far too small to see (ten-thousandths of a block) that says how deep the
water is there and how close the shore; the shader reads it back. Chunks meshed before the mod was
on wave once they are meshed again.

How the water and swaying work: Horizon meshes terrain once per chunk, and draws it without a shader
pack with a shader no mod can reach. So, with no shader pack picked and waves or swaying on, the game
runs the mod's own Horizon shader pack (`config/horizon_physics_pack`), which looks like vanilla. The
terrain shader the game builds around a Horizon pack gets the mod's vertex displacement (with a table
of which atlas sprites are water, leaves or plants) and, for the mod's own pack, the water's light.
Another Horizon pack (such as Aurelia) gets the waves and swaying and keeps its own water look. If the
shader does not parse, the game's own one is kept. An OptiFine-style `shaders/` pack is left alone:
nothing waves or sways under it. While the mod's pack runs, Horizon's improved transparency and the
chunk fade-in are off, as with any Horizon pack.

All of it is client-side decoration: nothing changes in the world or for other players. The pieces are
simulated with a small rigid-body solver (position-based dynamics) against the blocks around them.

Inspired by [PhysicsMod](https://modrinth.com/mod/physicsmod) by Haubna; no code or assets from it are
used. MIT licensed.

Pinned to Horizon `9d979caa48a3bb58203319b9ea044db58c38e0d8` and horizon-mixins `d9c83d7`.
