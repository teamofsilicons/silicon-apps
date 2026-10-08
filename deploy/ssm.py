#!/usr/bin/env python3
"""Run a reviewed, secret-free command file through the production SSM role."""
import argparse,json,subprocess,time
p=argparse.ArgumentParser();p.add_argument('instance');p.add_argument('script');p.add_argument('--timeout',type=int,default=600);a=p.parse_args()
def aws(*args):
 r=subprocess.run(['aws','--profile','silicon-production','--region','us-east-2','--no-cli-pager',*args],capture_output=True,text=True)
 if r.returncode:raise RuntimeError(r.stderr)
 return json.loads(r.stdout)
with open(a.script) as f: commands=f.read()
r=aws('ssm','send-command','--document-name','AWS-RunShellScript','--instance-ids',a.instance,'--parameters',json.dumps({'commands':[commands],'executionTimeout':[str(a.timeout)]}),'--comment','Silicon Apps deployment')
c=r['Command']['CommandId'];print('SSM command:',c,flush=True)
end=time.monotonic()+a.timeout+30
while time.monotonic()<end:
 time.sleep(3)
 try:r=aws('ssm','get-command-invocation','--command-id',c,'--instance-id',a.instance)
 except RuntimeError as e:
  if 'InvocationDoesNotExist' in str(e):continue
  raise
 if r['Status'] in ('Pending','InProgress','Delayed'):continue
 print(json.dumps({k:r.get(k) for k in ['Status','ResponseCode','StandardOutputContent','StandardErrorContent']},indent=2),flush=True)
 raise SystemExit(0 if r['Status']=='Success' else 1)
raise SystemExit('SSM execution did not finish within its bounded timeout.')
