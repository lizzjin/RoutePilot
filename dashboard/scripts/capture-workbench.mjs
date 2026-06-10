import { chromium } from 'playwright-core'
import fs from 'node:fs'
import path from 'node:path'
if (!process.env.ROUTEPILOT_ENV_FILE) throw new Error('Set ROUTEPILOT_ENV_FILE to a local test credential file')
const env = Object.fromEntries(fs.readFileSync(process.env.ROUTEPILOT_ENV_FILE,'utf8').trim().split('\n').map(line => { const split=line.indexOf('=');return [line.slice(0,split),line.slice(split+1)] }))
const base = process.env.PLAYWRIGHT_BASE_URL || 'http://127.0.0.1:33013'
const out=path.resolve('../docs/assets/workbench-redesign/implementation');fs.mkdirSync(out,{recursive:true})
const browser=await chromium.launch();const page=await browser.newPage({reducedMotion:'reduce'});const issues=[];page.on('pageerror',e=>issues.push(e.message))
for (const width of [1440,1024,390]) { await page.setViewportSize({width,height:width===390?844:1000}); await page.goto(base+'/login'); await page.locator('#username').waitFor(); await page.screenshot({path:path.join(out,'login-'+width+'.png')}) }
await page.goto(base+'/login');await page.locator('#username').fill(env.ROUTEPILOT_ADMIN_USERNAME);await page.locator('#password').fill(env.ROUTEPILOT_ADMIN_PASSWORD);await page.getByRole('button',{name:'登录',exact:true}).click();await page.waitForURL('**/dashboard');
for(const width of [1440,1024,390]){await page.setViewportSize({width,height:width===390?844:1000});for(const route of ['dashboard','logs','models','api-keys','users','quotas','operations','enterprise','governance','settings','guide','not-found']){await page.goto(base+'/'+route);await page.locator('h1').first().waitFor();await page.waitForLoadState('networkidle');await page.evaluate(()=>document.fonts.ready);const overflow=await page.evaluate(()=>({page:document.documentElement.scrollWidth,viewport:innerWidth}));if(overflow.page>overflow.viewport+1)issues.push({route,width,overflow});await page.screenshot({path:path.join(out,route+'-'+width+'.png')})}}
fs.writeFileSync(path.join(out,'capture-results.json'),JSON.stringify({issues},null,2));console.log(JSON.stringify({issues}));await browser.close()
