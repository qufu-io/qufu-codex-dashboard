import {invoke} from '@tauri-apps/api/core';
import {translate, type Language} from './i18n';

type Config={metric:'turns'|'context'|'five'|'week';selected:string|null;language:Language;theme:'dark'|'light'};
type RecordValue=Record<string,any>;
const esc=(v:unknown)=>String(v??'—').replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]!));
export async function mountHistory(){
 let config:Config=(window as any).__QUFU_HISTORY__??{metric:'turns',selected:null,language:'en',theme:'dark'};
 let data:any=null,problem=false,view='used',generation=0;
 const app=document.querySelector<HTMLDivElement>('#app')!;
 const t=(s:string)=>translate(s,config.language);
 const fmt=(v:unknown)=>typeof v==='number'&&Number.isFinite(v)?Intl.NumberFormat(config.language==='tr'?'tr-TR':'en-US',{maximumFractionDigits:2}).format(v):'—';
 const compact=(n:number)=>Intl.NumberFormat('en-US',{notation:'compact',maximumFractionDigits:1}).format(n);
 const stamp=(n:unknown)=>typeof n==='number'?new Date(n*1000).toLocaleString(config.language==='tr'?'tr-TR':'en-GB'):'—';
 const names={turns:'Görev tüketimi',context:'Context geçmişi',five:'5 saatlik kapasite geçmişi',week:'Haftalık kapasite geçmişi'};
 function detail(p:RecordValue){
  const u=p.usage;
  if(config.metric==='turns'||config.metric==='context')return `${t('Görev')} ${p.sequence} · ${stamp(p.completedAt)} · ${fmt(p.durationMs==null?null:p.durationMs/1000)} s\nInput ${fmt(u?.input)} · Cache ${fmt(u?.cache_missing?null:u?.cache)} · ${t('Cache dışı input')} ${fmt(u&&!u.cache_missing?Math.max(0,u.input-u.cache):null)} · Output ${fmt(u?.output)} · Reasoning ${fmt(u?.reasoning_missing?null:u?.reasoning)}\nUsed ${fmt(p.used)} · Raw ${fmt(p.rawTotal)} · ${p.usdPartial||p.usdApproximate?'≈ ':''}${fmt(p.usd)} $${p.usdPartial?' · '+t('Kısmi'):''} · ${p.context?`Context ${fmt(p.context.used)} / ${fmt(p.context.capacity)} · %${fmt(p.context.percent)}`:''}\n${[...new Set((p.responses??[]).map((r:any)=>`${r.model} / ${r.tier||'standard'}`))].join(', ')} · ${p.effort??'—'}${p.compacted?' · '+t('Compaction'):''}`;
  return `${stamp(p.at)} · ${p.plan??'—'} · ${p.limitId??'—'}\n${t('Referans kapasite')}: ${fmt(p.capacity)} · ${t('Gözlemler')}: ${fmt(p.intervals)} · ${t(p.reliability??'—')}\nN/C/O: ${(p.weights??[]).map((n:number)=>n.toExponential(3)).join(' / ')} · r: ${fmt(p.weeklyRatio)}${p.weeklyRatioLearned?'':' ('+t('Varsayım')+')'}\n${t('Tipik sapma')}: ${p.medianErrorPercent==null?'—':'%'+fmt(p.medianErrorPercent)} · p90: ${p.p90ErrorPercent==null?'—':'%'+fmt(p.p90ErrorPercent)}${p.changeAnalysis?' · '+t(p.changeAnalysis.status):''} · ${t('Resmi kullanım')}: ${p.percent==null?'—':'%'+fmt(p.percent)}`;
 }
 function render(){
  document.documentElement.lang=config.language;document.documentElement.dataset.theme=config.theme;document.body.className='history-body';
  const turns=config.metric==='turns',context=config.metric==='context',quota=!turns&&!context;
  const options=turns?[['used','CLI Used'],['raw','Raw Total'],['usd',t('Tahmini USD')]]:quota?[['capacity',t('Referans kapasite')],['percent',t('Resmi kullanım')]]:[];
  if(!options.some(o=>o[0]===view)&&options.length)view=options[0][0];
  app.innerHTML=`<section class="history-window"><div class="history-heading" data-tauri-drag-region><strong data-tauri-drag-region>${t(names[config.metric])}</strong><button class="icon" id="historyClose" aria-label="${t('Kapat')}"><svg viewBox="0 0 24 24"><path d="m6 6 12 12M6 18 18 6"/></svg></button></div><div class="history-controls">${options.length?`<select id="historyView" aria-label="${t('Grafik ölçümü')}">${options.map(([v,n])=>`<option value="${v}" ${view===v?'selected':''}>${n}</option>`).join('')}</select>`:`<span>${t('Görev sonunda ölçülen context')}</span>`}<span class="history-unit">${quota&&view==='capacity'?t('Cache dışı input eşdeğeri'):context||view==='percent'?'%':view==='usd'?'$':'Token'}</span></div><div id="historyPlot" class="history-plot"></div><p class="history-notice">${quota?t('Tahmini kapasite değişimi; backend kota değişimi olarak doğrulanamaz.'):t('Yalnızca tamamlanan görevler; eksik ölçümler doldurulmaz.')}</p><output id="historyDetail" class="history-detail">${t('Ayrıntı için bir noktaya veya çubuğa gel.')}</output></section>`;
  document.getElementById('historyClose')!.onclick=()=>void invoke('close_history');
  const select=document.getElementById('historyView') as HTMLSelectElement|null;
  if(select)select.onchange=()=>{view=select.value;render();};
  const plot=document.getElementById('historyPlot')!;
  if(problem||!data){plot.innerHTML=`<p class="empty">${t(problem?'Geçmiş okunamadı. Pencereyi kapatıp tekrar dene.':'Geçmiş yükleniyor…')}</p>`;return;}
  let records:RecordValue[];
  if(quota)records=(view==='percent'?data.points:data.fits).map((p:RecordValue)=>({...p,x:p.at,y:view==='percent'?p.percent:p.identifiable&&['validated','limited'].includes(p.reliability)?p.capacity:null}));
  else records=(data as RecordValue[]).filter(p=>p.status==='completed').map((p,i)=>({...p,sequence:i+1,x:i+1,y:context?p.context?.percent:view==='usd'?p.usd:view==='raw'?p.rawTotal:p.used}));
  if(!records.some(p=>typeof p.y==='number'&&Number.isFinite(p.y))){plot.innerHTML=`<p class="empty">${t(quota&&view==='capacity'?'Güvenilir kapasite geçmişi için yeterli gözlem yok.':'Bu grafik için ölçüm yok.')}</p>`;return;}
  // Large histories retain actual extrema/endpoints; this only reduces drawing, never stored measurements.
  if(records.length>400){const step=Math.ceil(records.length/160),reduced:RecordValue[]=[];for(let i=0;i<records.length;i+=step){const bucket=records.slice(i,i+step);const valid=bucket.filter(p=>typeof p.y==='number');const selected=new Set([bucket[0],bucket[bucket.length-1],...valid.sort((a,b)=>a.y-b.y).filter((_,j,a)=>j===0||j===a.length-1),...bucket.filter(p=>p.y==null)].filter(Boolean));reduced.push(...bucket.filter(p=>selected.has(p)));}records=reduced;}
  const values=records.filter(p=>typeof p.y==='number'&&Number.isFinite(p.y)),max=Math.max(...values.map(p=>p.y),view==='usd'?0.000001:1),first=records[0].x,last=records[records.length-1].x;
  const x=(v:number)=>43+(v-first)/Math.max(1,last-first)*370,y=(v:number)=>137-v/max*122;
  let chart=`<svg class="history-svg" viewBox="0 0 430 160" role="img" aria-label="${t(names[config.metric])}"><path class="history-axis" d="M43 10v127h370"/><text x="37" y="16" text-anchor="end">${view==='percent'||context?'%':''}${view==='usd'?max.toFixed(4):compact(max)}</text><text x="37" y="140" text-anchor="end">0</text><text x="43" y="156">${quota?esc(new Date(first*1000).toLocaleDateString(config.language)):first}</text><text x="413" y="156" text-anchor="end">${quota?esc(new Date(last*1000).toLocaleDateString(config.language)):last}</text>`;
  let previous:RecordValue|null=null;
  records.forEach((p,i)=>{
   if(typeof p.y!=='number'||!Number.isFinite(p.y)){previous=null;return;}
   const title=esc(detail(p)),xx=x(p.x),yy=y(p.y);
   if(turns){const width=Math.max(1,Math.min(18,340/records.length));chart+=`<rect class="history-mark" data-point="${i}" tabindex="0" x="${xx-width/2}" y="${yy}" width="${width}" height="${Math.max(1,137-yy)}"><title>${title}</title></rect>`;}
   else {
    const same=previous&&(context?previous.context?.capacity===p.context?.capacity&&previous.model===p.model:previous.plan===p.plan&&previous.limitId===p.limitId&&p.at-previous.at<=900&&(view==='percent'?previous.reset===p.reset:previous.reliability==='validated'&&p.reliability==='validated'));
    if(same)chart+=`<path class="history-line" d="M${x(previous!.x)} ${y(previous!.y)}L${xx} ${yy}"/>`;
    chart+=`<circle class="history-mark${p.compacted?' compacted':''}" data-point="${i}" tabindex="0" cx="${xx}" cy="${yy}" r="3"><title>${title}</title></circle>`;
   }
   previous=p;
  });
  plot.innerHTML=chart+'</svg>';
  const display=(e:Event)=>{const target=(e.target as Element).closest('[data-point]');if(!target)return;const p=records[Number(target.getAttribute('data-point'))];document.getElementById('historyDetail')!.textContent=detail(p);};
  plot.onpointerover=display;plot.addEventListener('focusin',display);plot.onclick=display;
 }
 async function load(){const n=++generation;problem=false;data=null;render();try{const result=await invoke('history',{metric:config.metric,selected:config.selected});if(n===generation)data=result;}catch{if(n===generation)problem=true;}if(n===generation)render();}
 window.addEventListener('history-config',e=>{config=(e as CustomEvent<Config>).detail;view='used';void load();});
 document.addEventListener('keydown',e=>{if(e.key==='Escape')void invoke('close_history');});
 await load();
}
