import json,pathlib,sys
root=pathlib.Path(__file__).resolve().parents[1]
report_path=pathlib.Path(sys.argv[1]) if len(sys.argv)>1 else root/'artifacts/private/diagnostics.json'
report=json.loads(report_path.read_text(encoding='utf-8'))
rates=json.loads((root/'src-tauri/prices.json').read_text(encoding='utf-8'))['rates']
results=[]
for detail in report['details']:
    thread=detail['thread'];last=None;seen=set();record_sum=0;first_baseline=0;observed=[];responses=[];costs=[];capacities=set();model="";tier=""
    for line in pathlib.Path(thread['path']).open(encoding='utf-8'):
        if not any(key in line for key in ['"token_usage_record"','"model_context_window"','"turn_context"','"thread_settings_applied"']):continue
        try:event=json.loads(line)
        except ValueError:continue
        payload=event.get('payload',{})
        if event.get('type')=='turn_context':
            model=payload.get('model','')
            if payload.get('service_tier') is not None:tier=payload['service_tier']
        if payload.get('type')=='thread_settings_applied':tier=(payload.get('thread_settings') or {}).get('service_tier') or tier
        capacity=payload.get('model_context_window') or (payload.get('info') or {}).get('model_context_window')
        if isinstance(capacity,int) and capacity>0:capacities.add(capacity)
        if event.get('type')!='token_usage_record':continue
        p=event.get('payload',{})
        if p.get('thread_id')!=thread['id'] or p.get('response_id') in seen:continue
        seen.add(p.get('response_id'));u=p.get('usage',{});responses.append(u);total=u.get('total_tokens',u.get('input_tokens',0)+u.get('output_tokens',0))
        normalized_tier={'':'standard','default':'standard','priority':'fast'}.get(tier,tier)
        rate=rates.get(f"{model}|{normalized_tier}|{'long' if u.get('input_tokens',0)>272000 else 'short'}")
        cache=u.get('cached_input_tokens',0);write=u.get('cache_write_input_tokens',0);inp=u.get('input_tokens',0);out=u.get('output_tokens',0)
        cost=None if not rate or cache+write>inp or (cache and rate.get('cache') is None) or (write and rate.get('write') is None) else ((inp-cache-write)*rate['input']+cache*(rate.get('cache') or 0)+write*(rate.get('write') or 0)+out*rate['output'])/1e6
        costs.append(cost)
        cumulative=p.get('thread_token_usage',{});cur=cumulative.get('total_tokens',cumulative.get('input_tokens',0)+cumulative.get('output_tokens',0))
        if last is None:first_baseline=cur-total
        record_sum+=total;last=cur;observed.append(cumulative)
    measured=detail['usage']['total'] if detail['usage'] else None
    fields={'input':'input_tokens','cache':'cached_input_tokens','write':'cache_write_input_tokens','output':'output_tokens','reasoning':'reasoning_output_tokens','total':'total_tokens'}
    passed=detail['usage'] is not None and any(all(detail['usage'][a]==v.get(b,0) for a,b in fields.items()) for v in observed)
    if thread['id']!=report['sessions'][0]['id']:passed=passed and measured==last
    latest=detail.get('lastUsage');context=detail.get('context')
    latest_matches=latest is None or any(all(latest[a]==v.get(b,v.get('input_tokens',0)+v.get('output_tokens',0) if b=='total_tokens' else 0) for a,b in fields.items()) for v in responses)
    context_matches=context is None or (context['capacity'] in capacities and any(context['used']==v.get('total_tokens',v.get('input_tokens',0)+v.get('output_tokens',0)) for v in responses) and abs(context['percent']-min(100,context['used']/context['capacity']*100))<1e-9)
    last_usd_matches=any(all(latest[a]==v.get(b,v.get('input_tokens',0)+v.get('output_tokens',0) if b=='total_tokens' else 0) for a,b in fields.items()) and (detail.get('lastUsd') is None and cost is None or detail.get('lastUsd') is not None and cost is not None and abs(detail['lastUsd']-cost)<1e-9) for v,cost in zip(responses,costs)) if latest else detail.get('lastUsd') is None
    passed=passed and latest_matches and context_matches and last_usd_matches
    results.append({'last_usd_matches':last_usd_matches,'last_response_matches':latest_matches,'context_matches':context_matches,'id':thread['id'],'reported':measured,'latest_record_total':last,'baseline_plus_responses':first_baseline+record_sum,'all_components_match_record':passed,'pass':passed})
reference_path=root/'artifacts/private/sidebar-reference.json'
if reference_path.exists():
    ref=json.loads(reference_path.read_text(encoding='utf-8'));actual={t['id'] for t in report['sessions']};expected={t['id'] for t in ref}
    discovery={'pass':actual==expected,'extra':sorted(actual-expected),'missing':sorted(expected-actual),'count':len(actual)}
else:discovery={'pass':False,'reason':'No independent Desktop sidebar reference captured'}
output={'session_totals':results,'sidebar':discovery,'scope':'local Codex threads only; cloud/ChatGPT not locally measurable'}
(root/'artifacts/real-data-verification.json').write_text(json.dumps(output,indent=2),encoding='utf-8')
print(json.dumps(output,indent=2))
sys.exit(0 if discovery['pass'] and all(r['pass'] for r in results) else 1)
