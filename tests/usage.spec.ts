import {test,expect} from '@playwright/test';
import {compactTokens,taskUsageWarning} from '../src/usage';

test('compact tokens use unambiguous English billion units',()=>{
 expect(compactTokens(3280000000)).toBe('3.28B');
 expect(compactTokens(164000000)).toBe('164M');
 expect(compactTokens(40000)).toBe('40K');
 expect(compactTokens(0)).toBe('0');
});
test('adaptive task warnings distinguish volume from uncached usage and unknown data',()=>{
 const baseline={count:5,rawMedian:500000,usedMedian:50000};
 expect(taskUsageWarning({rawTotal:500000,used:150000},baseline)).toMatchObject({kind:'used-spike',factor:3});
 expect(taskUsageWarning({rawTotal:1500000,used:30000},baseline)).toMatchObject({kind:'raw-spike',factor:3});
 expect(taskUsageWarning({rawTotal:5000000,used:20000},{count:0,rawMedian:null,usedMedian:null})).toMatchObject({kind:'raw-volume',factor:null});
 expect(taskUsageWarning({rawTotal:5000000,used:500000,usagePartial:true},baseline)).toMatchObject({kind:'raw-volume',partial:true});
 for(const task of [null,{rawTotal:null,used:null},{rawTotal:900000,used:99000},{rawTotal:2000000,used:200000,usagePartial:true}])expect(taskUsageWarning(task,baseline)).toBeNull();
 expect(taskUsageWarning({rawTotal:1500000,used:200000},{...baseline,count:4})).toBeNull();
 expect(taskUsageWarning({rawTotal:1500000,used:200000},{count:20,rawMedian:0,usedMedian:0})).toBeNull();
});
