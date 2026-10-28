import { test, expect } from '@playwright/test';
test.beforeEach(async({page})=>{
 await page.addInitScript(()=>{
  const usage={input:1000,cache:800,write:0,output:200,reasoning:50,total:1200};
  const sessions=[{id:'test-main',title:'Test ana session',workspace:'C:\\Test\\Workspace',group:'Test Workspace',updated:100,tokens:1200},{id:'test-other',title:'İkinci session',workspace:'C:\\Test\\Other',group:'Other',updated:90,tokens:1800}];
  let saved:any=JSON.parse(localStorage.getItem('test-startup-settings')??'{}');(window as any).__calls=[];
  (window as any).__TAURI_INTERNALS__={metadata:{currentWindow:{label:'main'},currentWebview:{label:'main'}},invoke:async(cmd:string,args:any)=>{
   (window as any).__calls.push({cmd,args});
   if(cmd==='load_settings')return saved;
   if(cmd==='window_preferences'&&args.height&&(window as any).__resizeNative){await (window as any).__resizeNative(args);return;}
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

test('account freshness uses clear update time and stale age labels',async({page})=>{
 await expect(page.locator('#source')).toHaveText(/^Last updated: \d{2}:\d{2}$/);
 await page.evaluate(()=>{const base=(window as any).__TAURI_INTERNALS__.invoke;(window as any).__TAURI_INTERNALS__.invoke=async(cmd:string,args:any)=>{const v=await base(cmd,args);if(cmd==='snapshot'){v.quota.stale=true;v.quota.at-=180;}return v;};});
 await page.getByRole('button',{name:'Refresh',exact:true}).click();
 await expect(page.locator('#source')).toHaveText('Limits may be out of date · 3 min ago');
 await page.locator('#settingsToggle').click();await fitPanel(page);await page.locator('#language').selectOption('tr');
 await expect(page.locator('#source')).toHaveText('Limitler güncel olmayabilir · 3 dk önce');
 await expect(page.getByRole('progressbar',{name:'5 saatlik hesap limiti'})).toHaveAttribute('aria-valuenow','42');
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
 await expect(page.locator('#five .quota-amount')).toHaveText('≈ 42M / 100M N equivalent');
 await expect(page.locator('#five .quota-amount')).toHaveAttribute('title',/not an official token quota/);
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
 await expect(page.locator('#contextUsage')).toHaveClass('history-link usage-warning');
 await expect(page.locator('#five .limit-top')).toHaveClass(/usage-warning/);await expect(page.locator('#week .limit-top')).toHaveClass(/usage-warning/);
 const colors=await page.evaluate(()=>['#contextUsage .track>span','#five .track>span','#week .track>span'].map(s=>getComputedStyle(document.querySelector(s)!).backgroundColor));
 expect(new Set(colors).size).toBe(1);expect(colors[0]).toBe('rgb(238, 164, 137)');expect(page.viewportSize()!.height).toBe(height);
 await page.screenshot({path:'artifacts/ui-usage-warning-1.0.png'});
 await page.locator('#settingsToggle').click();await fitPanel(page);await page.locator('#theme').selectOption('light');
 await expect(page.locator('#contextUsage .track>span')).toHaveCSS('background-color','rgb(163, 68, 34)');
 await page.evaluate(()=>{(window as any).__warningValues={context:25,quota:30};});await page.locator('#refresh').click();
 await expect(page.locator('.usage-warning')).toHaveCount(0);await expect(page.locator('.track>span.danger')).toHaveCount(0);
});


test('updates check current and new releases without login, and install only after a click',async({page})=>{
 await page.evaluate(()=>{const base=(window as any).__TAURI_INTERNALS__.invoke;(window as any).__updateReply={available:false,currentVersion:'1.0.0'};(window as any).__TAURI_INTERNALS__.invoke=async(cmd:string,args:any)=>{if(cmd==='check_update'){(window as any).__calls.push({cmd,args});return (window as any).__updateReply;}return base(cmd,args);};});
 await page.locator('#settingsToggle').click();await fitPanel(page);
 await expect(page.locator('#updateStatus')).toHaveText('');
 await expect(page.locator('#updateStatus')).toHaveAttribute('title',/No GitHub account needed/);
 await page.getByRole('button',{name:'Check for updates',exact:true}).click();
 await expect(page.locator('#updateStatus')).toHaveText('Up to date');
 await page.evaluate(()=>{(window as any).__updateReply={available:true,currentVersion:'1.0.0',version:'1.1.0'};});
 await page.locator('#checkUpdate').click();await expect(page.locator('#updateStatus')).toHaveText('New version: 1.1.0');
 expect(await page.evaluate(()=>(window as any).__calls.filter((c:any)=>c.cmd==='install_update').length)).toBe(0);
 await fitPanel(page);await page.screenshot({path:'artifacts/ui-update-available-1.0.png'});
 await page.locator('#checkUpdate').click();await expect(page.locator('#updateDetails')).toContainText('Update installed');
 expect(await page.evaluate(()=>(window as any).__calls.filter((c:any)=>c.cmd==='install_update').length)).toBe(1);
 await page.locator('#openReleases').click();
 expect(await page.evaluate(()=>(window as any).__calls.filter((c:any)=>c.cmd==='open_release_page').length)).toBe(1);
 await page.locator('#language').selectOption('tr');await fitPanel(page);
 await expect(page.locator('#updatesHeading')).toHaveText('Sürüm');
 await expect(page.locator('#checkUpdate')).toHaveText('Güncellemeleri kontrol et');
 await page.screenshot({path:'artifacts/ui-update-settings-1.0-tr.png'});
});

test('update failures stay retryable and never claim the app is current',async({page})=>{
 await page.evaluate(()=>{const base=(window as any).__TAURI_INTERNALS__.invoke;(window as any).__TAURI_INTERNALS__.invoke=async(cmd:string,args:any)=>{if(cmd==='check_update')throw new Error('offline or no published manifest');return base(cmd,args);};});
 await page.locator('#settingsToggle').click();await fitPanel(page);await page.locator('#checkUpdate').click();
 await expect(page.locator('#updateDetails')).toContainText('Could not check for updates');
 await expect(page.locator('#checkUpdate')).toBeEnabled();await expect(page.locator('#updateStatus')).not.toContainText('latest version');
 await fitPanel(page);
});


test('compact settings saves height and keeps controls aligned at 300 and 371 pixels',async({page})=>{
 await page.evaluate(()=>{const base=(window as any).__TAURI_INTERNALS__.invoke;(window as any).__TAURI_INTERNALS__.invoke=async(cmd:string,args:any)=>cmd==='check_update'?{available:false,currentVersion:'1.0.0'}:base(cmd,args);});
 await page.locator('#settingsToggle').click();await fitPanel(page);
 await expect(page.locator('#panel')).not.toContainText('Auto Follow keeps your manual selection');
 await expect(page.locator('#panel')).toContainText('API prices');await expect(page.locator('#panel')).toContainText('not your subscription bill');
 const before=JSON.parse(await (await import('node:fs/promises')).readFile('artifacts/private/settings-before.json','utf8').catch(()=>'{"panel":535}'));
 await page.locator('#checkUpdate').click();await expect(page.locator('#updateStatus')).toHaveText('Up to date');
 const height=await page.locator('#panel').evaluate(e=>e.getBoundingClientRect().height);expect(height).toBeLessThan(400);
 await (await import('node:fs/promises')).writeFile('artifacts/settings-size-comparison.json',JSON.stringify({before:before.panel,after:height,saved:before.panel-height}));
 for(const width of [300,371]){
  await page.setViewportSize({width,height:1200});
  for(const language of ['en','tr']){await page.locator('#language').selectOption(language);for(const theme of ['dark','light']){
   await page.locator('#theme').selectOption(theme);
   expect(await page.evaluate(()=>document.documentElement.scrollWidth<=window.innerWidth)).toBe(true);
   const layout=await page.evaluate(()=>{const box=(id:string)=>document.querySelector(id)!.getBoundingClientRect();return {check:box('#checkUpdate').y,github:box('#openReleases').y,status:box('#updateStatus').y,author:box('.author').bottom,bottom:box('#releaseFooter').bottom};});
   expect(Math.abs(layout.check-layout.github)).toBeLessThan(1);expect(Math.abs(layout.check-layout.status)).toBeLessThan(6);expect(layout.author).toBeLessThanOrEqual(layout.bottom);
   await expect(page.locator('.author')).toContainText('alper@qufu.io');
  }}
  await page.locator('#app').screenshot({path:`artifacts/ui-settings-compact-${width}.png`});
 }
});
test('USD fallback and incomplete agents remain visible as approximate partial data',async({page})=>{
 await page.evaluate(()=>{const base=(window as any).__TAURI_INTERNALS__.invoke;(window as any).__TAURI_INTERNALS__.invoke=async(cmd:string,args:any)=>{const v=await base(cmd,args);if(cmd==='snapshot'&&v.detail){v.detail.usdApproximate=true;v.detail.usdPartial=true;v.detail.lastUsdApproximate=true;v.detail.children=[{id:'a',title:'A',usage:{total:200},usd:0.001,status:'Working'},{id:'b',title:'B',usage:{total:300},usd:0.002,status:'Working'},{id:'c',title:'C',usage:null,usd:null,status:'Unavailable'}];v.detail.childUsage={total:500};v.detail.childUsd=0.003;v.detail.childUsdPartial=true;v.detail.childrenComplete=false;}return v;};});
 await page.locator('#sessionTitle').click();await page.locator('[data-id="test-main"]').click();
 await expect(page.locator('.estimated-price strong')).toHaveText('≈ 0.0025 $');await expect(page.locator('.last-price strong')).toHaveText('≈ 0.0006 $');
 await page.locator('#detailsToggle').click();await fitPanel(page);await expect(page.locator('#agents')).toContainText('500');await expect(page.locator('#agents .partial-label')).toHaveText('Partial');
 await expect(page.locator('#panel table').last()).toContainText('Unavailable');await expect(page.locator('#panel table').last()).toContainText('200');await expect(page.locator('#panel table').last()).toContainText('300');
});
test('unvalidated capacity never invents errors and retains measured local tokens',async({page})=>{
 await page.evaluate(()=>{const base=(window as any).__TAURI_INTERNALS__.invoke;(window as any).__TAURI_INTERNALS__.invoke=async(cmd:string,args:any)=>{const v=await base(cmd,args);if(cmd==='snapshot')v.quota.value.primary.token_estimate={used:null,capacity:null,localTokens:40000,approximate:true,intervals:0,resets:0,medianErrorPercent:null,p90ErrorPercent:null,reliability:'insufficient'};return v;};});
 await page.locator('#refresh').click();await expect(page.locator('#five .quota-amount')).toHaveText('40K / — Token');await expect(page.locator('#five .quota-amount')).not.toHaveAttribute('title',/Typical error/);await expect(page.locator('#five .limit-top strong')).toHaveText('%42');
});


test('metric history opens a separate window with the selected session and appearance',async({page})=>{
 await page.getByRole('button',{name:'Open session list'}).click();await page.locator('[data-id="test-main"]').click();
 for(const [label,metric] of [['Task usage','turns'],['Context history','context'],['5-hour capacity history','five'],['Weekly capacity history','week']]){
  await page.getByRole('button',{name:label,exact:true}).click();
  expect(await page.evaluate(()=> (window as any).__calls.filter((c:any)=>c.cmd==='open_history_window').at(-1))).toMatchObject({args:{metric,selected:'test-main',language:'en',theme:'dark'}});
 }
});
for(const language of ['en','tr']) for(const metric of ['turns','context','five','week']) test(`history ${metric} ${language} fits, preserves gaps and exposes actual measurements`,async({page})=>{
 await page.addInitScript(({language,metric})=>{
  (window as any).__QUFU_HISTORY__={metric,selected:'test-main',language,theme:language==='en'?'dark':'light'};
  const usage={input:1000,cache:800,output:200,reasoning:50,total:1200};
  const turns=[{id:'one',status:'completed',startedAt:100,completedAt:120,durationMs:20000,usage,used:400,rawTotal:1200,usd:0.003,model:'test',responses:[{model:'gpt-6.1-sol',tier:'standard'}],context:{used:1000,capacity:128000,percent:0}},{id:'missing',status:'completed',startedAt:200,completedAt:220,usage:null,used:null,rawTotal:null,usd:null,context:null},{id:'three',status:'completed',startedAt:300,completedAt:320,usage,used:500,rawTotal:1300,usd:0.004,compacted:true,model:'test',context:{used:70000,capacity:128000,percent:50}},{id:'active',status:'in_progress',usage}];
  const fits=[{at:100,capacity:2000000,identifiable:true,reliability:'validated',weights:[0.00005,0.00001,0.0002],weeklyRatio:0.15,weeklyRatioLearned:false,intervals:30,medianErrorPercent:5,p90ErrorPercent:15,plan:'pro',limitId:'codex'},{at:200,capacity:3000000,identifiable:false,reliability:'learning'},{at:300,capacity:2500000,identifiable:true,reliability:'validated',plan:'pro',limitId:'codex'}];
  (window as any).__historyCalls=[];
  (window as any).__TAURI_INTERNALS__={invoke:async(cmd:string,args:any)=>{(window as any).__historyCalls.push({cmd,args});if(cmd==='history')return metric==='turns'||metric==='context'?turns:{fits,points:[{at:100,percent:35,reset:900,plan:'pro',limitId:'codex'},{at:300,percent:42,reset:900,plan:'pro',limitId:'codex'}]};return null;}};
 },{language,metric});
 await page.setViewportSize({width:480,height:300});await page.goto('http://127.0.0.1:5173/?history=1');
 await expect(page.locator('.history-mark')).toHaveCount(2);
 expect(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth||document.documentElement.scrollHeight>innerHeight)).toBe(false);
 await page.locator('.history-mark').first().focus();await expect(page.locator('#historyDetail')).not.toHaveText('');
 if(metric==='turns'){await expect(page.locator('#historyView')).toHaveValue('used');await expect(page.locator('#historyDetail')).toContainText('Used 400');await page.locator('#historyView').selectOption('usd');await expect(page.locator('#historyDetail')).not.toContainText('NaN');}
 if(metric==='context')expect(await page.locator('.history-line').count()).toBe(0);
 if(metric==='five'||metric==='week'){expect(await page.locator('.history-line').count()).toBe(0);await page.locator('#historyView').selectOption('percent');await expect(page.locator('.history-mark').first().locator('title')).toContainText('%35');}
 await page.setViewportSize({width:320,height:240});expect(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth||document.documentElement.scrollHeight>innerHeight)).toBe(false);
 expect(await page.evaluate(()=>(window as any).__historyCalls.some((c:any)=>['save_settings','window_preferences','snapshot'].includes(c.cmd)))).toBe(false);
 await page.screenshot({path:`artifacts/history-${metric}-${language}.png`});
 await page.locator('#historyClose').click();expect(await page.evaluate(()=>(window as any).__historyCalls.at(-1).cmd)).toBe('close_history');
});
test('history read failure is distinct from empty or zero measurements',async({page})=>{
 await page.addInitScript(()=>{(window as any).__QUFU_HISTORY__={metric:'turns',selected:'x',language:'en',theme:'dark'};(window as any).__TAURI_INTERNALS__={invoke:async()=>{throw Error('fixture');}};});
 await page.goto('http://127.0.0.1:5173/?history=1');await expect(page.locator('#historyPlot')).toContainText('History unavailable');await expect(page.locator('.history-mark')).toHaveCount(0);
});

test('large history reduction preserves every missing context boundary',async({page})=>{
 await page.addInitScript(()=>{
  (window as any).__QUFU_HISTORY__={metric:'context',selected:'x',language:'en',theme:'dark'};
  const rows=Array.from({length:651},(_,i)=>({id:String(i),status:'completed',startedAt:i+1,completedAt:i+2,model:'same',context:i===1||i===3?null:{percent:i===2?100:i===0?1:2,used:13000,capacity:128000}}));
  (window as any).__TAURI_INTERNALS__={invoke:async(cmd:string)=>cmd==='history'?rows:null};
 });
 await page.setViewportSize({width:480,height:300});await page.goto('http://127.0.0.1:5173/?history=1');
 await expect(page.locator('.history-mark').first()).toBeVisible();
 const gap=43+3/650*370;
 const paths=await page.locator('.history-line').evaluateAll(nodes=>nodes.map(n=>n.getAttribute('d')!));
 for(const d of paths){const [,start,,end]=d.match(/M([\d.]+) ([\d.]+)L([\d.]+) ([\d.]+)/)!;expect(Number(start)<gap&&Number(end)>gap).toBe(false);}
});

test('whole-task usage distinguishes current and completed tasks while preserving session scope',async({page})=>{
 await page.evaluate(()=>{const base=(window as any).__TAURI_INTERNALS__.invoke;(window as any).__taskStatus='in_progress';(window as any).__TAURI_INTERNALS__.invoke=async(cmd:string,args:any)=>{const v=await base(cmd,args);if(cmd==='snapshot'&&v.detail){const status=(window as any).__taskStatus;v.detail.lastUsage={input:300,cache:200,output:30,reasoning:10,total:330};v.detail.used=400;v.detail.rawTotal=1200;v.detail.turn={id:'whole-task',status,used:130,rawTotal:330,usagePartial:true};v.detail.lastCompletedTurn={used:status==='completed'?130:80,rawTotal:status==='completed'?330:100};v.detail.lastUsdPartial=true;}return v;};});
 await page.locator('#sessionTitle').click();await page.locator('[data-id="test-main"]').click();await fitWindow(page);
 await expect(page.locator('.total-tokens')).toContainText('1,200');await expect(page.locator('.last-request')).toContainText('330');await expect(page.locator('.last-request .partial-label')).toHaveText('Partial');
 await expect(page.locator('.turn-state')).toContainText('In progress');await expect(page.locator('.turn-state')).toContainText('CLI Used 130');await expect(page.locator('.last-price')).toContainText('≈');
 expect(await page.evaluate(()=>document.documentElement.scrollHeight>innerHeight||document.documentElement.scrollWidth>innerWidth)).toBe(false);
 await page.locator('#detailsToggle').click();await expect(page.locator('.detail-facts')).toContainText('80 Used');
 await page.evaluate(()=>{(window as any).__taskStatus='completed';});await page.locator('#refresh').click();await expect(page.locator('.turn-state')).toContainText('Completed');await expect(page.locator('.detail-facts')).toContainText('130 Used');
});


test('startup fits the window without opening settings and retires compact preference',async({page})=>{
 await page.exposeFunction('__resizeNative',async(size:{width:number;height:number})=>{await page.setViewportSize(size);});
 for(const compact of [false,true]){
  await page.evaluate(compact=>localStorage.setItem('test-startup-settings',JSON.stringify({schemaVersion:5,selected:'test-main',compact,language:'en',theme:'dark',opacity:0.55,topmost:false,autoFollow:false})),compact);
  await page.setViewportSize({width:820,height:290});
  await page.reload();
  await expect(page.locator('#sessionTitle')).toContainText('Test ana session');
  await expect.poll(()=>page.evaluate(()=>window.innerWidth===300&&window.innerHeight===Math.ceil(document.querySelector('#app')!.getBoundingClientRect().height)&&document.documentElement.scrollHeight<=window.innerHeight)).toBe(true);
  const initial=page.viewportSize();
  await page.setViewportSize({width:600,height:900});
  await expect.poll(()=>page.viewportSize()).toEqual(initial);
  await page.locator('#settingsToggle').click();
  await expect(page.locator('#compact')).toHaveCount(0);
  await expect(page.locator('#transparency')).toHaveValue('45');
  await expect.poll(()=>page.viewportSize()?.width).toBe(371);
  await page.locator('#settingsToggle').click();
  await expect.poll(()=>page.viewportSize()).toEqual(initial);
  expect(await page.evaluate(()=>(window as any).__saved.compact)).toBeUndefined();
 }
});


test('public README screenshot uses illustrative data without personal information',async({page})=>{
 await page.evaluate(()=>{
  const base=(window as any).__TAURI_INTERNALS__.invoke;
  (window as any).__TAURI_INTERNALS__.invoke=async(cmd:string,args:any)=>{
   const value=await base(cmd,args);
   if(cmd!=='snapshot')return value;
   const usage={input:1240000,cache:980000,write:0,output:60000,reasoning:12000,total:1300000};
   value.sessions=[{id:'example',title:'Dashboard',workspace:'C:\\Projects\\Example',group:'Example Workspace',updated:100,tokens:1300000}];
   value.detail={...value.detail,thread:value.sessions[0],usage,rawTotal:1300000,used:320000,lastUsage:{input:24000,cache:18000,write:0,output:1600,reasoning:300,total:25600},lastUsd:0.0256,usd:1.48,tpm:2400,model:'example',status:'Tamamlandı',models:[],warnings:[],children:[],childUsage:null,childUsd:null,childrenComplete:true,turn:{id:'example-turn',status:'completed',usagePartial:false,used:7600,rawTotal:25600,durationMs:18000,effort:'medium'},context:{used:72000,capacity:128000,percent:52},contextCapacity:128000};
   value.quota.value.primary.token_estimate=null;
   value.quota.value.secondary.token_estimate=null;
   return value;
  };
 });
 await page.locator('#sessionTitle').click();
 await page.locator('[data-id="test-main"]').click();
 await fitWindow(page);
 await expect(page.locator('#sessionTitle')).toHaveText('Example Workspace - Dashboard');
 await expect(page.locator('#workspace')).toHaveText('C:\\Projects\\Example');
 expect((await page.locator('#app').innerText()).replaceAll('alper@qufu.io','')).not.toMatch(/alper|GenA|IOG|Users|Test ana/);
 await page.locator('#app').screenshot({path:'artifacts/dashboard-public-1.0.3.png'});
});


test('version, updates and author stay visible without Settings',async({page})=>{
 await expect(page.locator('.author')).toBeVisible();
 await expect(page.locator('.author')).toHaveText('qufu.ioalper@qufu.io');
 await expect(page.locator('#updatesHeading')).toHaveText('Version');
 await expect(page.locator('#checkUpdate')).toBeVisible();
 await page.evaluate(()=>{const base=(window as any).__TAURI_INTERNALS__.invoke;(window as any).__TAURI_INTERNALS__.invoke=async(cmd:string,args:any)=>cmd==='check_update'?{available:false,currentVersion:'1.0.3'}:base(cmd,args);});
 await page.locator('#checkUpdate').click();
 await expect(page.locator('#updateStatus')).toHaveText('Up to date');
 await page.locator('#settingsToggle').click();
 await fitPanel(page);
 await page.locator('#language').selectOption('tr');
 await expect(page.locator('#updatesHeading')).toHaveText('Sürüm');
 await fitPanel(page);
 await page.locator('#settingsToggle').click();
 await expect(page.locator('.author')).toBeVisible();
 await expect(page.locator('#checkUpdate')).toHaveText('Güncellemeleri kontrol et');
 await fitWindow(page);
});
