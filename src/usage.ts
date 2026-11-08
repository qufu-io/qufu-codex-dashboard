export type TaskUsage={rawTotal?:number|null;used?:number|null;usagePartial?:boolean};
export type TaskBaseline={count:number;rawMedian:number|null;usedMedian:number|null};
export function compactTokens(value:number):string {
 return Intl.NumberFormat('en-US',{notation:'compact',maximumFractionDigits:2}).format(value);
}
const known=(n:unknown):n is number=>typeof n==='number'&&Number.isFinite(n)&&n>=0;
export function taskUsageWarning(task:TaskUsage|undefined|null,baseline?:TaskBaseline|null){
 if(!task)return null;
 const comparable=!task.usagePartial&&baseline&&baseline.count>=5;
 if(comparable&&known(task.used)&&task.used>=100000&&known(baseline.usedMedian)&&baseline.usedMedian>0&&task.used>=3*baseline.usedMedian)
  return {kind:'used-spike' as const,value:task.used,factor:task.used/baseline.usedMedian,partial:false};
 if(comparable&&known(task.rawTotal)&&task.rawTotal>=1000000&&known(baseline.rawMedian)&&baseline.rawMedian>0&&task.rawTotal>=3*baseline.rawMedian)
  return {kind:'raw-spike' as const,value:task.rawTotal,factor:task.rawTotal/baseline.rawMedian,partial:false};
 if(known(task.rawTotal)&&task.rawTotal>=5000000)
  return {kind:'raw-volume' as const,value:task.rawTotal,factor:null,partial:!!task.usagePartial};
 return null;
}
