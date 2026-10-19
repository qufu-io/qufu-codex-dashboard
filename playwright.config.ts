import { defineConfig } from '@playwright/test';
export default defineConfig({testDir:'tests',testMatch:'*.spec.ts',use:{channel:'msedge',headless:true,viewport:{width:300,height:400}},webServer:{command:'npm.cmd run dev -- --port 5173',url:'http://127.0.0.1:5173',reuseExistingServer:false},workers:1});
