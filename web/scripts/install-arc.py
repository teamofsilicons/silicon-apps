import json,subprocess,pathlib
root=pathlib.Path(__file__).resolve().parents[1]
seen=set()
def fetch(url):return subprocess.check_output(['curl','-fsSL','--user-agent','Mozilla/5.0',url]).decode()
def install(name):
 if name in seen:return
 seen.add(name)
 d=json.loads(fetch('https://uiarc.dev/r/'+name+'.json'))
 (root/'vendor/uiarc'/f'{name}.json').write_text(json.dumps(d,indent=2)+'\n')
 print(name, d.get('dependencies',[]))
 for dep in d.get('registryDependencies',[]):install(dep.rsplit('/',1)[-1].removesuffix('.json'))
 for f in d.get('files',[]):
  target=f.get('target','').replace('@components/','src/components/')
  if not target:raise ValueError(f)
  p=root/target
  p.parent.mkdir(parents=True,exist_ok=True)
  p.write_text(f['content'])
 if not name.startswith('arc-'):
  (root/'vendor/uiarc/docs'/f'{name}.md').write_text(fetch('https://uiarc.dev/components/'+name+'/markdown'))
for name in ['arc-foundation','button','input','textarea','search-field','badge','empty-state','dialog','switch','copy-button','stepper','segmented-control']:install(name)
(root/'vendor/uiarc/docs/SKILL.md').write_text(fetch('https://uiarc.dev/r/skills/arc/SKILL.md'))
