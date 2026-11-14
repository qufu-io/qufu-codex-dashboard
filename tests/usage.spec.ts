import {test,expect} from '@playwright/test';
import {compactTokens} from '../src/usage';

test('compact tokens use unambiguous English billion units',()=>{
 expect(compactTokens(3280000000)).toBe('3.28B');
 expect(compactTokens(164000000)).toBe('164M');
 expect(compactTokens(40000)).toBe('40K');
 expect(compactTokens(0)).toBe('0');
});
