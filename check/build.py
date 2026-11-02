import re, pathlib, subprocess, sys
here = pathlib.Path(__file__).parent
b = here.parent / 'mixins' / 'horizon_physics'
t = (here / 'template.rs').read_text()
t = t.replace('//RUNTIME//', (b / 'runtime.rs').read_text())
t = t.replace('//SWAY_MODULE//', (b / 'sway.rs').read_text())
for tag, f, name in [('MOB','mob.rs','mob'),('PART','part.rs','part'),('DESTROY','destroy.rs','destroy'),
                     ('LISTED','listed.rs','listed'),('SETTINGS','settings.rs','settings'),
                     ('SETTINGS_CLICK','settings_click.rs','settings_click'),('WORLD','world.rs','world'),('ITEMS','items.rs','items'),('EXPLODE','explode.rs','explode'),('AIRED','aired.rs','aired'),('LEADS','leads.rs','leads'),('FISHING','fishing.rs','fishing'),('CAPE','cape.rs','cape'),('SHADER','shader.rs','shader'),('SHADER_PACK','shader_pack.rs','shader_pack'),('BUILTIN','builtin.rs','builtin'),('CAMPFIRE_SMOKE','campfire_smoke.rs','campfire_smoke'),('FIRE_SMOKE','fire_smoke.rs','fire_smoke'),('FLUID','fluid.rs','fluid'),('BOATS','boats.rs','boats')]:
    src = (b / f).read_text()
    src = re.sub(r'\bfn horizon_physics_\w+\(', f'fn __h_{name}(', src, count=1)
    t = t.replace(f'//{tag}//', src)
(here / 'check.rs').write_text(t)
subprocess.run(['rustc','--edition','2024','--crate-type','lib','--crate-name','glam','glam.rs','-o','/tmp/libglam.rlib'],cwd=here,check=True)
subprocess.run(['rustc','--edition','2024','--crate-type','lib','--crate-name','naga','naga.rs','-o','/tmp/libnaga.rlib'],cwd=here,check=True)
r = subprocess.run(['rustc','--extern','glam=/tmp/libglam.rlib','--extern','naga=/tmp/libnaga.rlib', '--edition', '2024', '--crate-type', 'lib', '--crate-name', 'horizon_client', 'check.rs', '-o', '/tmp/hp_check.rlib'], cwd=here, capture_output=True, text=True)
print(r.stderr[-6000:]); sys.exit(r.returncode)
