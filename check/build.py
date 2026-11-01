import re, pathlib, subprocess, sys
here = pathlib.Path(__file__).parent
b = here.parent / 'mixins' / 'horizon_physics'
t = (here / 'template.rs').read_text()
t = t.replace('//RUNTIME//', (b / 'runtime.rs').read_text())
for tag, f, name in [('MOB','mob.rs','mob'),('PART','part.rs','part'),('DESTROY','destroy.rs','destroy'),
                     ('LISTED','listed.rs','listed'),('SETTINGS','settings.rs','settings'),
                     ('SETTINGS_CLICK','settings_click.rs','settings_click'),('WORLD','world.rs','world'),('ITEMS','items.rs','items')]:
    src = (b / f).read_text()
    src = re.sub(r'\bfn horizon_physics_\w+\(', f'fn __h_{name}(', src, count=1)
    t = t.replace(f'//{tag}//', src)
(here / 'check.rs').write_text(t)
subprocess.run(['rustc','--edition','2024','--crate-type','lib','--crate-name','glam','glam.rs','-o','/tmp/libglam.rlib'],cwd=here,check=True)
r = subprocess.run(['rustc','--extern','glam=/tmp/libglam.rlib', '--edition', '2024', '--crate-type', 'lib', '--crate-name', 'horizon_client', 'check.rs', '-o', '/tmp/hp_check.rlib'], cwd=here, capture_output=True, text=True)
print(r.stderr[-6000:]); sys.exit(r.returncode)
