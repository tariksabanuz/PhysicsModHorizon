# Horizon Physics

A native mod for [Horizon](https://github.com/horizon-rust/horizon) (Minecraft Java 26.3 in Rust) that
brings PhysicsMod-style physics to the client, written from scratch:

- **Crumbling blocks**: a broken block falls apart into pieces of its own texture that tumble, bounce,
  pile up and fade after a while.
- **Mob ragdolls**: a mob that dies drops as a ragdoll: its model parts (head, body, arms, legs) stay
  joined where they meet and fall limply.
- **Mobs falling apart**: or the parts come loose and tumble separately (or a mix of both).
- **Item physics**: dropped items tumble while they fly and lie flat on the ground instead of floating
  and spinning.
- **Explosions**: TNT, creepers, beds and the rest blow their blocks apart: the pieces are thrown out
  of the blast, with a fireball, sparks that bounce and smoke that rises and drifts. Pieces lying
  around and mobs killed by the blast are thrown too.
- **Smoke and fire**: fire, soul fire, lit campfires and open lava give off smoke that rises, spreads
  under ceilings and drifts with the wind.
- **Snow footprints**: walking on snow leaves footprints, left and right, that fade after 30 seconds.
- **Ropes**: leads and fishing lines sag, swing and lie on the blocks.
- **Cloth capes**: capes are cloth that trails, flutters in the wind and hangs down when you stand.
- **Water waves and swaying plants** (with a Horizon shader pack, such as Aurelia): the water's
  surface moves in waves; leaves, grass, flowers, crops, tall plants, vines, seagrass and kelp sway,
  more in wind, rain and thunder. Lava moves slowly too.
- **Liquids (beta)**: water or lava you pour splashes out as droplets that run over the blocks before
  they settle; falling into water splashes.
- **Wind**: calm, a breeze or strong, for smoke, capes, ropes and the swaying.
- **Gravity**: Earth, Moon, Mars, Pluto or none, for the pieces.

Settings: Options > Mods > Horizon Physics (saved in `config/horizon_physics.txt`).

How the waves and swaying work: Horizon meshes terrain once per chunk, and its shader packs have no
vertex stage. When a Horizon shader pack runs, the mod adds a vertex displacement to the terrain shader
the game builds around the pack, with a table of which atlas sprites are water, leaves or plants. If
that shader does not parse, the pack keeps the game's own one. Without a Horizon shader pack (vanilla
look, or an OptiFine-style `shaders/` pack) nothing waves or sways. Changing these settings rebuilds the
pack (the chat says "Shader pack ... reloaded").

All of it is client-side decoration: nothing changes in the world or for other players. The pieces are
simulated with a small rigid-body solver (position-based dynamics) against the blocks around them.

Inspired by [PhysicsMod](https://modrinth.com/mod/physicsmod) by Haubna; no code or assets from it are
used. MIT licensed.

Pinned to Horizon `9d979caa48a3bb58203319b9ea044db58c38e0d8` and horizon-mixins `d9c83d7`.
