import { test, expect } from '@playwright/test';
test.beforeEach(async({page})=>{
 await page.addInitScript(()=>{
  const usage={input:1000,cache:800,write:0,output:200,reasoning:50,total:1200};
  const sessions=[{id:'test-main',title:'Test ana session',workspace:'C:\\Test\\Workspace',group:'Test Workspace',updated:100,tokens:1200},{id:'test-other',title:'İkinci session',workspace:'C:\\Test\\Other',group:'Other',updated:90,tokens:1800}];
  let saved:any={};(window as any).__calls=[];
  (window as any).__TAURI_INTERNALS__={metadata:{currentWindow:{label:'main'},currentWebview:{label:'main'}},invoke:async(cmd:string,args:any)=>{
   (window as any).__calls.push({cmd,args});
   if(cmd==='load_settings')return saved;
   if(cmd==='save_settings'){saved=args.settings;(window as any).__saved=saved;return;}

   if(cmd==='snapshot')return {sessions,detail:args.selected?{thread:sessions.find(t=>t.id===args.selected),usage,lastUsage:{input:400,output:50,cache:300,write:0,reasoning:20,total:450},context:{used:450,capacity:1000,percent:45},contextCapacity:1000,lastUsd:0.00055,usd:0.00248,tpm:1200,model:'gpt-6.1-sol',status:'Tamamlandı',models:[{model:'gpt-6.1-sol',usage,usd:0.00248}],warnings:[],children:[],childUsage:{...usage,input:0,cache:0,output:0,total:0},childUsd:0,childrenComplete:true}:null,quota:{at:Math.floor(Date.now()/1000),stale:false,source:'Codex account/rateLimits/read',value:{plan_type:"pro",primary:{used_percent:42,window_minutes:300,token_estimate:{used:42000000,capacity:100000000,localTokens:39000000,approximate:true,intervals:294,resets:42,medianErrorPercent:20,p90ErrorPercent:71.2},resets_at:Math.floor(Date.now()/1000)+3600},secondary:{used_percent:68,window_minutes:10080,resets_at:Math.floor(Date.now()/1000)+86400}}},excluded:{internal:4},errors:[],at:100,follow:{id:null,reason:'Seçim doğrulanamadı'}};
   return null;
  }};
 });
 await page.goto('http://127.0.0.1:5173');
 await fitWindow(page);
});
test('manual selection, metrics, details and settings',async({page})=>{
 await expect(page.locator('#status')).toHaveText('— tok/s');
 await expect(page.locator('#metrics')).toContainText('—');
 await page.getByRole('button',{name:'Open session list'}).click();
 await expect(page.locator('.session-row')).toHaveCount(2);
 await expect(page.getByRole('searchbox')).toHaveCount(0);
 await expect(page.locator('[data-id="test-main"]')).toHaveText('Test Workspace - Test ana session');
 await page.locator('[data-id="test-main"]').click();
 await expect(page.locator('#sessionTitle')).toHaveText('Test Workspace - Test ana session');
 await expect(page.locator('#workspace')).toHaveText('C:\\Test\\Workspace');
 await expect(page.locator('main > #agents')).toHaveCount(0);
 await expect(page.locator('#metrics')).toContainText('1,200');
 await expect(page.locator('#status')).toHaveText('20 tok/s');
 await fitWindow(page);
 await expect(page.getByRole('progressbar',{name:'5-hour account limit'})).toHaveAttribute('aria-valuenow','42');
 await page.screenshot({path:'artifacts/ui-compact-test.png'});
 expect(await page.evaluate(()=>document.documentElement.scrollWidth>window.innerWidth)).toBe(false);
 expect(await page.evaluate(()=>document.documentElement.scrollHeight>window.innerHeight)).toBe(false);
 await page.getByRole('button',{name:'Details'}).click();
 await expect(page.getByRole('heading',{name:'Session details'})).toBeVisible();
 await expect(page.locator('#panel #agents')).toBeVisible();
 await expect(page.locator('#panel #agents')).toContainText('Subagents');
 await expect(page.locator('.detail-facts')).toContainText('Account subscription');
 await expect(page.locator('.detail-facts')).toContainText('Pro');
 await expect(page.locator('.detail-facts')).toContainText('1,000');
 await fitPanel(page);
 await page.screenshot({path:'artifacts/ui-details-test.png'});
 await page.getByRole('button',{name:'Settings',exact:true}).click();
 await fitPanel(page);
 await expect(page.locator('#autoFollow')).not.toBeChecked();
 await page.locator('#autoFollow').check();
 await page.getByRole('button',{name:'Refresh',exact:true}).click();
 await expect(page.locator('#sessionTitle')).toHaveText('Test Workspace - Test ana session');
 await expect(page.locator('#workspace')).toHaveText('C:\\Test\\Workspace');
 await expect(page.locator('main > #agents')).toHaveCount(0);
 await page.locator('#theme').selectOption('light');
 await expect(page.locator('html')).toHaveAttribute('data-theme','light');
});
test('read errors never invent zero usage',async({page})=>{
 await page.evaluate(()=>{(window as any).__TAURI_INTERNALS__.invoke=async(cmd:string)=>{if(cmd==='snapshot')throw new Error('test read failure');return null;};});
 await page.getByRole('button',{name:'Refresh',exact:true}).click();
 await expect(page.getByRole('alert')).toContainText('Could not read Codex usage');
 await expect(page.locator('#metrics')).toContainText('—');
});

test('transparency default and refresh never reapply window geometry',async({page})=>{
 await expect.poll(()=>page.evaluate(()=>(window as any).__saved?.opacity)).toBe(0.67);
 await page.getByRole('button',{name:'Settings',exact:true}).click();
 await expect(page.locator('#transparency')).toHaveValue('33');
 await expect(page.locator('#transparencyValue')).toHaveText('%33');
 await fitPanel(page);
 const count=await page.evaluate(()=>(window as any).__calls.filter((c:any)=>c.cmd==='window_preferences').length);
 await page.getByRole('button',{name:'Refresh',exact:true}).click();
 await expect(page.getByRole('heading',{name:'Settings',exact:true})).toBeVisible();
 expect(await page.evaluate(()=>(window as any).__calls.filter((c:any)=>c.cmd==='window_preferences').length)).toBe(count);
 await page.locator('#topmost').check();
 await expect.poll(()=>page.evaluate(()=>(window as any).__saved?.topmost)).toBe(true);
 const change=await page.evaluate(()=>(window as any).__calls.filter((c:any)=>c.cmd==='window_preferences').at(-1).args);
 expect(change).toEqual({topmost:true,alpha:0.67});
 await page.locator('#theme').selectOption('light');
 expect(await page.evaluate(()=>(window as any).__calls.filter((c:any)=>c.cmd==='window_preferences').length)).toBe(count+1);
 await page.getByRole('button',{name:'Close',exact:true}).click();
 const close=await page.evaluate(()=>(window as any).__calls.filter((c:any)=>c.cmd==='window_preferences').at(-1).args);
 expect(close.width).toBe(300);
 expect(close.height).toBe(await page.locator('#app').evaluate(e=>Math.ceil(e.getBoundingClientRect().height)));
});

test('only a fresh live account response is labelled Live',async({page})=>{
 await expect(page.locator('#source')).toContainText('Live');
 await page.evaluate(()=>{const base=(window as any).__TAURI_INTERNALS__.invoke;(window as any).__TAURI_INTERNALS__.invoke=async(cmd:string,args:any)=>{const v=await base(cmd,args);if(cmd==='snapshot'){v.quota.stale=true;v.quota.at-=180;}return v;};});
 await page.getByRole('button',{name:'Refresh',exact:true}).click();
 await expect(page.locator('#source')).not.toContainText('Live');
 await expect(page.getByRole('progressbar',{name:'5-hour account limit'})).toHaveAttribute('aria-valuenow','42');
});

test('quiet logo, no startup control, English and Turkish persist',async({page})=>{
 await expect(page.locator('header')).not.toContainText('Qufu Codex Dashboard');
 expect(await page.locator('#logo').evaluate(e=>e.closest('button,a')===null)).toBe(true);
 const box=await page.locator('#logo').boundingBox();await page.mouse.click(box!.x+box!.width/2,box!.y+box!.height/2);
 await expect(page.locator('#drawer')).toBeHidden();
 await page.getByRole('button',{name:'Settings',exact:true}).click();
 await expect(page.locator('#autostart')).toHaveCount(0);
 await expect(page.locator('.author')).toContainText('qufu.io');
 await expect(page.locator('.author')).toContainText('alper@qufu.io');
 await expect(page.locator('html')).toHaveAttribute('lang','en');
 await expect(page.locator('#language')).toHaveValue('en');
 await expect(page.getByRole('heading',{name:'Settings',exact:true})).toBeVisible();
 await expect(page.locator('html')).toHaveAttribute('lang','en');
 await expect(page.getByRole('progressbar',{name:'5-hour account limit'})).toBeVisible();
 await expect.poll(()=>page.evaluate(()=>(window as any).__saved?.language)).toBe('en');
 await fitPanel(page);
 await page.screenshot({path:'artifacts/ui-settings-1.0-en.png'});
 await page.locator('#language').selectOption('tr');
 await expect(page.getByRole('heading',{name:'Ayarlar',exact:true})).toBeVisible();
 await fitPanel(page);
 await page.screenshot({path:'artifacts/ui-settings-1.0-tr.png'});
});

async function fitWindow(page:import('@playwright/test').Page){
 await expect.poll(()=>page.evaluate(()=>(window as any).__calls.filter((c:any)=>c.cmd==='window_preferences'&&c.args.height).length)).toBeGreaterThan(0);
 for(let pass=0;pass<2;pass++){
  await expect.poll(()=>page.evaluate(()=>{const size=(window as any).__calls.filter((c:any)=>c.cmd==='window_preferences'&&c.args.height).at(-1).args;return size.height===Math.ceil(document.querySelector('#app')!.getBoundingClientRect().height);})).toBe(true);
  const size=await page.evaluate(()=>(window as any).__calls.filter((c:any)=>c.cmd==='window_preferences'&&c.args.height).at(-1).args);
  await page.setViewportSize({width:size.width??371,height:size.height});
 }
 await expect.poll(()=>page.evaluate(()=>document.documentElement.scrollHeight<=window.innerHeight)).toBe(true);
 expect(await page.evaluate(()=>document.documentElement.scrollWidth<=window.innerWidth)).toBe(true);
}
async function fitPanel(page:import('@playwright/test').Page){
 await expect.poll(()=>page.evaluate(()=>(window as any).__calls.filter((c:any)=>c.cmd==='window_preferences'&&c.args.width===371).length)).toBeGreaterThan(0);
 await fitWindow(page);
 expect(await page.locator('#panel').evaluate(e=>getComputedStyle(e).overflowY)).toBe('visible');
}

test('vertical limits, expanding panels and footer typography',async({page})=>{
 const five=await page.locator('#five').boundingBox(),week=await page.locator('#week').boundingBox();
 expect(week!.y).toBeGreaterThan(five!.y+five!.height);
 expect(week!.x).toBe(five!.x);
 await page.locator('#sessionTitle').click();
 await page.locator('[data-id="test-main"]').click();
 await page.locator('#detailsToggle').click();
 await fitPanel(page);
 await page.locator('#settingsToggle').click();
 await fitPanel(page);
 const fonts=await page.evaluate(()=>{const note=getComputedStyle(document.querySelector('.note')!),author=getComputedStyle(document.querySelector('.author')!);return {note:note.fontSize,author:author.fontSize,style:author.fontStyle,label:getComputedStyle(document.querySelector('.settings-grid label')!).fontSize,select:getComputedStyle(document.querySelector('#language')!).paddingRight};});
 expect(fonts.author).toBe(fonts.note);
 expect(fonts.style).toBe('italic');
 expect(fonts.label).toBe(fonts.note);
 expect(parseFloat(fonts.select)).toBeGreaterThanOrEqual(20);
 await page.screenshot({path:'artifacts/ui-vertical-settings-1.0.png'});
});

test('compact content fits at minimum width with the requested metric order',async({page})=>{
 expect(page.viewportSize()!.width).toBe(300);
 expect(page.viewportSize()!.height).toBe(await page.locator('#app').evaluate(e=>Math.ceil(e.getBoundingClientRect().height)));
 await page.locator('#sessionTitle').click();
 await page.locator('[data-id="test-main"]').click();
 await fitWindow(page);
 await expect(page.locator('.metrics-session')).toHaveText('Test Workspace - Test ana session');
 await expect(page.locator('.total-tokens')).toContainText('1,200');
 await expect(page.locator('.token-breakdown .metric > span')).toHaveText(['Input','Output','Cache','Input','Output','Cache']);
 await expect(page.locator('.last-request')).toContainText('450');
 await expect(page.locator('.last-price')).toContainText('0.0006 $');
 await expect(page.locator('.session-price')).toHaveCount(0);
 const totalBox=await page.locator('.total-tokens').boundingBox(),priceBox=await page.locator('.estimated-price').boundingBox();
 expect(totalBox!.y).toBe(priceBox!.y);
 const lastBox=await page.locator('.last-request').boundingBox(),lastPriceBox=await page.locator('.last-price').boundingBox();
 expect(lastBox!.y).toBe(lastPriceBox!.y);
 await expect(page.locator('.last-breakdown strong')).toHaveText(['400','50','300']);
 await expect(page.locator('.total-summary .estimated-price')).toContainText('Estimated USD');
 await expect(page.locator('#status')).toHaveText('20 tok/s');
 await expect(page.getByRole('progressbar',{name:'Context usage',exact:true})).toHaveAttribute('aria-valuenow','45');
 const positions=await page.locator('#metrics > div').evaluateAll(elements=>elements.map(e=>e.getBoundingClientRect().top));
 for(let i=1;i<positions.length;i++)expect(positions[i]).toBeGreaterThan(positions[i-1]);
 const contextHeight=await page.locator('.context-track').evaluate(e=>e.getBoundingClientRect().height);
 expect(contextHeight).toBeGreaterThan(await page.locator('#five .track').evaluate(e=>e.getBoundingClientRect().height));
 const fonts=await page.evaluate(()=>({total:getComputedStyle(document.querySelector('.total-tokens strong')!).fontSize,small:getComputedStyle(document.querySelector('.token-breakdown strong')!).fontSize,bold:getComputedStyle(document.querySelector('.total-tokens strong')!).fontWeight}));
 expect(parseFloat(fonts.total)).toBeGreaterThan(parseFloat(fonts.small));expect(Number(fonts.bold)).toBeGreaterThanOrEqual(700);
 const before=await page.evaluate(()=>(window as any).__calls.filter((c:any)=>c.cmd==='window_preferences').length);
 await page.locator('#pin').click();
 await expect(page.locator('#pin')).toHaveAttribute('aria-pressed','true');
 const change=await page.evaluate(()=>(window as any).__calls.filter((c:any)=>c.cmd==='window_preferences').at(-1).args);
 expect(change).toEqual({topmost:true,alpha:0.67});
 expect(await page.evaluate(()=>(window as any).__calls.filter((c:any)=>c.cmd==='window_preferences').length)).toBe(before+1);
 await page.screenshot({path:'artifacts/ui-minimum-compact-1.0.png'});
});

test('missing latest usage and context stay unknown independently of session totals',async({page})=>{
 await page.evaluate(()=>{const base=(window as any).__TAURI_INTERNALS__.invoke;(window as any).__TAURI_INTERNALS__.invoke=async(cmd:string,args:any)=>{const v=await base(cmd,args);if(cmd==='snapshot'){if(v.detail){v.detail.lastUsage=null;v.detail.lastUsd=null;v.detail.context=null;v.detail.contextCapacity=null;}delete v.quota.value.plan_type;}return v;};});
 await page.locator('#sessionTitle').click();await page.locator('[data-id="test-main"]').click();await fitWindow(page);
 await expect(page.locator('.total-tokens')).toContainText('1,200');
 await expect(page.locator('.last-request strong')).toHaveText('—');
 await expect(page.locator('.last-price strong')).toHaveText('—');
 await expect(page.locator('.last-breakdown strong')).toHaveText(['—','—','—']);
 await expect(page.locator('#contextUsage strong')).toHaveText('—');
 await expect(page.getByRole('progressbar',{name:'Context usage',exact:true})).not.toHaveAttribute('aria-valuenow');
 await page.locator('#detailsToggle').click();await fitPanel(page);
 await expect(page.locator('.detail-facts')).toContainText('Unknown');
});

test('subagent totals appear only in details',async({page})=>{
 await page.evaluate(()=>{const base=(window as any).__TAURI_INTERNALS__.invoke;(window as any).__TAURI_INTERNALS__.invoke=async(cmd:string,args:any)=>{const v=await base(cmd,args);if(cmd==='snapshot'&&v.detail){v.detail.children=[{id:'agent-1',title:'Test worker',usage:{input:90,cache:30,write:0,output:10,reasoning:0,total:100},usd:0.001,status:'Tamamlandı',internal:false}];v.detail.childUsage={input:90,cache:30,write:0,output:10,reasoning:0,total:100};v.detail.childUsd=0.001;}return v;};});
 await page.locator('#sessionTitle').click();await page.locator('[data-id="test-main"]').click();await fitWindow(page);
 await expect(page.locator('#agents')).toHaveCount(0);
 await page.locator('#detailsToggle').click();await fitPanel(page);
 await expect(page.locator('#panel #agents')).toContainText('100');
 await expect(page.locator('#panel #agents')).toContainText('0.0010 $');
 await expect(page.locator('#panel')).toContainText('Test worker');
 await page.locator('#closePanel').click();await fitWindow(page);
 await expect(page.locator('#agents')).toBeHidden();
});

test('approved presentation uses prefix percentages and suffix dollars in both languages',async({page})=>{
 await page.locator('#sessionTitle').click();await page.locator('[data-id="test-main"]').click();await fitWindow(page);
 await expect(page.locator('#hide')).toHaveCount(0);
 await expect(page.getByRole('button',{name:'Hide to tray'})).toHaveCount(0);
 await expect(page.locator('#contextUsage')).toContainText('Context usage (450/1,000)');
 await expect(page.locator('#contextUsage strong')).toHaveText('%45.0');
 await expect(page.locator('#five .limit-top strong')).toHaveText('%42');
 await expect(page.locator('#week .limit-top strong')).toHaveText('%68');
 await expect(page.locator('.estimated-price strong')).toHaveText('0.0025 $');
 await expect(page.locator('.last-price strong')).toHaveText('0.0006 $');
 await page.locator('#settingsToggle').click();await fitPanel(page);await page.locator('#language').selectOption('tr');await fitPanel(page);
 await expect(page.locator('#transparencyValue')).toHaveText('%33');
 await expect(page.locator('#contextUsage strong')).toHaveText('%45.0');
 await expect(page.locator('#five .limit-top strong')).toHaveText('%42');
 await expect(page.locator('.estimated-price strong')).toHaveText('0.0025 $');
});

test('quota amounts are labeled estimates and missing calibration stays unknown',async({page})=>{
 await expect(page.locator('#five .quota-amount')).toHaveText('≈(42M/100M Token)');
 await expect(page.locator('#five .quota-amount')).toHaveAttribute('title',/not an official or fixed token quota/);
 await expect(page.locator('#five .quota-amount')).toHaveAttribute('title',/Typical error: %20.0/);
 await expect(page.locator('#week .quota-amount')).toHaveText('(—/— Token)');
 await expect(page.locator('#week .limit-top strong')).toHaveText('%68');
 await page.evaluate(()=>{const base=(window as any).__TAURI_INTERNALS__.invoke;(window as any).__TAURI_INTERNALS__.invoke=async(cmd:string,args:any)=>{const v=await base(cmd,args);if(cmd==='snapshot')v.quota.value.primary.token_estimate=null;return v;};});
 await page.locator('#refresh').click();await fitWindow(page);
 await expect(page.locator('#five .quota-amount')).toHaveText('(—/— Token)');
 await expect(page.locator('#five .limit-top strong')).toHaveText('%42');
});


test('usage warning thresholds preserve layout and reset after consumption drops',async({page})=>{
 await page.evaluate(()=>{const base=(window as any).__TAURI_INTERNALS__.invoke;(window as any).__warningValues={context:80,quota:90};(window as any).__TAURI_INTERNALS__.invoke=async(cmd:string,args:any)=>{const v=await base(cmd,args);if(cmd==='snapshot'){const n=(window as any).__warningValues;if(v.detail)v.detail.context={used:n.context*10,capacity:1000,percent:n.context};v.quota.value.primary.used_percent=n.quota;v.quota.value.secondary.used_percent=n.quota;}return v;};});
 await page.locator('#sessionTitle').click();await page.locator('[data-id="test-main"]').click();await fitWindow(page);
 await expect(page.locator('.metrics-divider')).toHaveCount(1);
 const total=await page.locator('.total-breakdown').boundingBox(),line=await page.locator('.metrics-divider').boundingBox(),request=await page.locator('.request-summary').boundingBox();
 expect(line!.y).toBeGreaterThan(total!.y+total!.height);expect(request!.y).toBeGreaterThan(line!.y);
 await expect(page.locator('.usage-warning')).toHaveCount(0);
 const height=page.viewportSize()!.height;
 await page.evaluate(()=>{(window as any).__warningValues={context:80.1,quota:90.1};});await page.locator('#refresh').click();await fitWindow(page);
 await expect(page.locator('#contextUsage')).toHaveClass('usage-warning');
 await expect(page.locator('#five .limit-top')).toHaveClass(/usage-warning/);await expect(page.locator('#week .limit-top')).toHaveClass(/usage-warning/);
 const colors=await page.evaluate(()=>['#contextUsage .track>span','#five .track>span','#week .track>span'].map(s=>getComputedStyle(document.querySelector(s)!).backgroundColor));
 expect(new Set(colors).size).toBe(1);expect(colors[0]).toBe('rgb(238, 164, 137)');expect(page.viewportSize()!.height).toBe(height);
 await page.screenshot({path:'artifacts/ui-usage-warning-1.0.png'});
 await page.locator('#settingsToggle').click();await fitPanel(page);await page.locator('#theme').selectOption('light');
 await expect(page.locator('#contextUsage .track>span')).toHaveCSS('background-color','rgb(163, 68, 34)');
 await page.evaluate(()=>{(window as any).__warningValues={context:25,quota:30};});await page.locator('#refresh').click();
 await expect(page.locator('.usage-warning')).toHaveCount(0);await expect(page.locator('.track>span.danger')).toHaveCount(0);
});
