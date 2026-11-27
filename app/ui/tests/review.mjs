import { installReviewFixture } from './fixture.mjs';
import { chromium, expect } from '@playwright/test';
import { createServer } from 'node:http';
import { createHash } from 'node:crypto';
import { readFile, stat } from 'node:fs/promises';
import { resolve, extname } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(fileURLToPath(new URL('../dist/', import.meta.url)));
const siteRoot = resolve(fileURLToPath(new URL('../../../site/', import.meta.url)));
const config = JSON.parse(await readFile(new URL('../../src-tauri/tauri.conf.json',import.meta.url),'utf8'));
const index = await readFile(resolve(root,'index.html'),'utf8');
// Tauri adds hashes for bundled inline bootstrap scripts when it serves the app assets.
const hashes = [...index.matchAll(/<script\b[^>]*>([\s\S]*?)<\/script>/gi)]
  .filter(match => match[1].trim()).map(match => "'sha256-"+createHash('sha256').update(match[1]).digest('base64')+"'");
const productionCsp = config.app.security.csp.replace("script-src 'self'","script-src 'self' "+hashes.join(' '));
const server = createServer(async (request, response) => {
  const pathname = new URL(request.url, 'http://localhost').pathname;
  if (pathname === '/favicon.ico') { response.writeHead(204).end(); return; }
  const isSite = pathname.startsWith('/download/');
  const source = isSite ? siteRoot : root;
  const suffix = isSite ? pathname.slice('/download'.length) : pathname;
  const path = resolve(source, '.' + suffix);
  if (path !== source && !path.startsWith(source + '/')) { response.writeHead(403).end(); return; }
  try {
    const file = (await stat(path)).isDirectory() ? resolve(path, 'index.html') : path;
    const types = { '.html': 'text/html', '.js': 'application/javascript', '.wasm': 'application/wasm', '.css': 'text/css', '.webp': 'image/webp' };
    response.setHeader('Content-Type', types[extname(file)] || 'application/octet-stream');
    if (extname(file) === '.html' && !isSite) response.setHeader('Content-Security-Policy',productionCsp);
    response.end(await readFile(file));
  } catch { response.writeHead(404).end(); }
});
await new Promise(done => server.listen(0, '127.0.0.1', done));
const url = `http://127.0.0.1:${server.address().port}`;
const wav = Buffer.alloc(44 + 6400 * 2);
wav.write('RIFF'); wav.writeUInt32LE(wav.length - 8, 4); wav.write('WAVEfmt ', 8);
wav.writeUInt32LE(16, 16); wav.writeUInt16LE(1, 20); wav.writeUInt16LE(1, 22);
wav.writeUInt32LE(16000, 24); wav.writeUInt32LE(32000, 28); wav.writeUInt16LE(2, 32); wav.writeUInt16LE(16, 34);
wav.write('data', 36); wav.writeUInt32LE(wav.length - 44, 40);
const browser = await chromium.launch({
  ...(process.env.SEAMONE_CHROME ? { executablePath: process.env.SEAMONE_CHROME } : {}),
  args: ['--autoplay-policy=no-user-gesture-required', '--mute-audio'],
});
let completed = 0;
async function captureReview(page, state) {
  if (process.env.SEAMONE_UI_SCREENSHOTS) {
    await page.screenshot({ path:resolve(process.env.SEAMONE_UI_SCREENSHOTS,`seamone-${state}.png`) });
  }
}
async function fixture({ infoSeen = true, invalidFirst = false, quick = false, long = false, branch = false, microphone = false, phone = false, openRecording = true } = {}) {
  const page = await browser.newPage(phone ? {viewport:{width:390,height:844},hasTouch:true,isMobile:true} : {});
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  page.on('console', message => { if (message.type()==='error') errors.push(message.text()); });
  await page.addInitScript(installReviewFixture, { wav: wav.toString('base64'), infoSeen, invalidFirst, quick, long, branch, microphone });
  await page.goto(url);
  if (!openRecording) return { page, errors };
  try { await page.getByRole('button', { name: '시험 녹음.wav', exact: true }).click({timeout:10000}); }
  catch (error) { throw new Error([...errors,String(error)].join('\n')); }
  await expect(page.getByText(quick ? '첫 번째 최신전사문' : '첫 번째 자동 전사문', { exact: true })).toBeVisible();
  return { page, errors };
}
async function finish(page, errors, name) {
  expect(errors, name).toEqual([]); await page.close(); completed++; console.log(`PASS ${name}`);
}
async function startBinary(page) {
  await page.getByText('조각별 검증', {exact:true}).click();
  await page.getByRole('button',{ name:'자동 전사문 Yes/No 검증',exact:true }).click();
}

const startQuick = page => page.getByRole('button',{ name:'빠른 검수 시작',exact:true }).click();
const bar = (page, kind, start, end) => page.locator(
  `.timeline-bar.${kind}[data-start="${start}"][data-end="${end}"]`);
const clipRanges = (page, kind) => page.locator(`.timeline-bar.${kind}`).evaluateAll(nodes =>
  nodes.map(node => [Number(node.dataset.start), Number(node.dataset.end)]));
const readState = (page, key) => page.evaluate(key => window.__REVIEW_TEST__[key],key);
async function seekAndPause(page, seconds) {
  await expect.poll(async () => (await readState(page,'playCalls')).length).toBeGreaterThan(0);
  const at = await page.evaluate(async seconds => {
    const state = window.__REVIEW_TEST__, session = state.output.session;
    const at = state.outputRange.start + Math.floor(seconds * 16000);
    await window.__TAURI__.core.invoke('control_playback',{session,action:{kind:'seek',at}});
    await window.__TAURI__.core.invoke('control_playback',{session,action:{kind:'pause'}});
    await window.__TAURI__.core.invoke('get_playback_status',{});
    return at;
  },seconds);
  await expect(page.locator('.timeline-playhead')).toHaveAttribute('data-sample',String(at));
}
try {
  {
    const {page,errors} = await fixture();
    await startBinary(page);
    await page.evaluate(() => { window.__REVIEW_TEST__.autoOutput = false; });
    await page.getByRole('button',{name:'구간 재생 (Space)',exact:true}).click();
    const slider = page.getByRole('slider',{name:'현재 조각 재생 위치',exact:true});
    await expect(slider).toBeVisible();
    await page.evaluate(() => { window.__REVIEW_TEST__.controlCalls = []; });
    const pollingBeforeDrag = await readState(page,'statusPolls');
    await slider.evaluate(node => {
      node.value = '3200'; node.dispatchEvent(new Event('input',{bubbles:true}));
    });
    await expect.poll(() => slider.inputValue()).toBe('3200');
    await expect.poll(() => page.evaluate(() => window.__REVIEW_TEST__.controlCalls.length)).toBe(0);
    await expect.poll(() => readState(page,'statusPolls')).toBeGreaterThan(pollingBeforeDrag + 1);
    await expect(slider).toHaveValue('3200');
    await slider.evaluate(node => node.dispatchEvent(new Event('change',{bubbles:true})));
    await expect.poll(async () => (await readState(page,'controlCalls')).map(call => call.action)).toEqual([{kind:'seek',at:3200}]);
    await expect.poll(() => readState(page,'output')).toMatchObject({position:3200});
    await slider.evaluate(node => {
      node.dispatchEvent(new PointerEvent('pointerdown',{bubbles:true}));
      node.dispatchEvent(new PointerEvent('pointerup',{bubbles:true}));
      window.__REVIEW_TEST__.output.position = 4000;
    });
    await expect(slider).toHaveValue('4000');
    await page.evaluate(() => {
      window.__REVIEW_TEST__.controlCalls = [];
      window.__REVIEW_TEST__.holdControl = true;
    });
    const toggle = page.locator('.runtime-playback button').first();
    await toggle.click();
    await expect.poll(async () => (await readState(page,'controlCalls')).map(call => call.action)).toEqual([{kind:'pause'}]);
    await toggle.click();
    await page.getByRole('button',{name:'재생 멈추기',exact:true}).click();
    expect((await readState(page,'controlCalls')).map(call => call.action)).toEqual([{kind:'pause'}]);
    await page.evaluate(() => window.__REVIEW_TEST__.releaseControl());
    await expect.poll(async () => (await readState(page,'controlCalls')).map(call => call.action)).toEqual([{kind:'pause'},{kind:'play'},{kind:'stop'}]);
    await expect.poll(() => readState(page,'output')).toMatchObject({state:'paused'});
    await finish(page,errors,'slider commits once and controls remain ordered while an IPC reply is pending');
  }
  {
    const {page, errors} = await fixture({microphone: true});
    const start = page.getByRole('button', {name:'녹음 시작', exact:true});
    const stop = page.getByRole('button', {name:'멈추고 저장', exact:true});
    const cancel = page.getByRole('button', {name:'녹음 취소', exact:true});
    await start.click();
    await expect(stop).toBeDisabled();
    await page.evaluate(() => { window.__REVIEW_TEST__.microphone = {state:'failed', message:'마이크 접근이 허용되지 않았습니다.'}; });
    await expect(page.getByText('마이크 접근이 허용되지 않았습니다.', {exact:true})).toBeVisible();

    await start.click();
    await cancel.click();
    await expect(page.getByText('녹음을 취소하는 중입니다.')).toBeVisible();
    await page.evaluate(() => window.__REVIEW_TEST__.releaseMicFinish());
    await expect(page.getByText('녹음을 취소했습니다.')).toBeVisible();
    await expect(page.getByRole('button', {name:'마이크 녹음.wav'})).toHaveCount(0);

    await start.click();
    await page.evaluate(() => { window.__REVIEW_TEST__.microphone = {state:'recording', session_id:'microphone-session', elapsed_seconds:12}; });
    await expect(page.getByText('녹음 중 00:12 / 30:00')).toBeVisible();
    await page.evaluate(() => { window.__REVIEW_TEST__.microphone = {state:'interrupted', session_id:'microphone-session', elapsed_seconds:12, reason:'마이크 연결이 끊겼습니다.'}; });
    await expect(page.getByText(/마이크 연결이 끊겼습니다.*저장하거나 취소한 뒤 창을 닫을 수 있습니다/)).toBeVisible();
    await page.evaluate(() => { window.__REVIEW_TEST__.failMicSave = true; });
    await stop.click();
    await expect(page.getByText('녹음을 저장하는 중입니다.')).toBeVisible();
    await expect(cancel).toBeDisabled();
    await page.evaluate(() => window.__REVIEW_TEST__.releaseMicFinish());
    await expect(page.getByRole('alert').filter({hasText:'시험 저장 실패'})).toBeVisible();
    await stop.click();
    await expect(page.getByText('녹음을 저장하는 중입니다.')).toBeVisible();
    await page.evaluate(() => window.__REVIEW_TEST__.releaseMicFinish());
    await expect(page.getByRole('button', {name:'마이크 녹음.wav'})).toHaveAttribute('aria-pressed','true');

    await start.click();
    await expect(cancel).toBeEnabled();
    await page.evaluate(() => { window.__REVIEW_TEST__.microphone = {state:'limit_reached', session_id:'microphone-session'}; });
    await expect(page.getByText(/30분 제한에 도달했습니다/)).toBeVisible();
    await cancel.click();
    await expect(page.getByText('녹음을 취소하는 중입니다.')).toBeVisible();
    await page.evaluate(() => window.__REVIEW_TEST__.releaseMicFinish());
    await expect(start).toBeEnabled();
    expect(await page.evaluate(() => window.__REVIEW_TEST__.finishCalls.map(c => c.save))).toEqual([false,true,true,false]);
    await finish(page,errors,'desktop microphone controls, interruption, retry and duration limit');
  }
  {
    const {page, errors} = await fixture({microphone: true});
    await page.getByRole('button', {name:'녹음 시작', exact:true}).click();
    await page.evaluate(() => { window.__REVIEW_TEST__.microphone = {state:'recording', session_id:'microphone-session', elapsed_seconds:12}; });
    const stop = page.getByRole('button', {name:'멈추고 저장', exact:true});
    await expect(stop).toBeEnabled();
    await page.getByRole('button', {name:'이 조각 재생, 편집'}).first().click();
    const draft = page.getByLabel('수정 초안');
    await draft.fill('아직 저장하지 않은 편집');
    await stop.click();
    await expect(page.getByText('녹음을 저장하는 중입니다.')).toBeVisible();
    await page.evaluate(() => window.__REVIEW_TEST__.releaseMicFinish());
    await expect(page.getByRole('status').filter({hasText:'녹음 보관함에 저장했습니다.'})).toBeVisible();
    await expect(draft).toHaveValue('아직 저장하지 않은 편집');
    await draft.press('Escape');
    await expect(page.getByRole('button', {name:'마이크 녹음.wav'})).toBeVisible();
    await finish(page,errors,'saving a recording leaves the open transcript draft in place');
  }
  {
    const {page, errors} = await fixture();
    await expect(page.getByRole('heading', {name:'마이크 녹음'})).toHaveCount(0);
    const importButton = page.getByRole('button', {name:'음성 파일 가져오기'});
    await importButton.click();
    await expect(importButton).toBeEnabled();
    await page.evaluate(() => { window.__REVIEW_TEST__.nextImport = '선택한 음성.m4a'; });
    await importButton.click();
    await expect(page.getByRole('button', {name:'선택한 음성.m4a'})).toHaveAttribute('aria-pressed','true');
    await expect(page.getByRole('button', {name:'기기 안에서 받아쓰기'})).toBeEnabled();
    await finish(page,errors,'file picker cancellation and successful import with microphone unavailable');
  }
  {
    const { page, errors } = await fixture({ infoSeen:false });
    await expect(page.getByRole('heading', { name:'기기에 남는 음성과 글' })).toBeVisible();
    await page.getByRole('button',{ name:'텍스트 파일로 저장',exact:true }).click();
    await expect.poll(() => page.evaluate(() => window.__REVIEW_TEST__.exports)).toBe(1);
    await page.getByRole('button',{ name:'저장한 글 복사',exact:true }).click();
    await expect.poll(() => page.evaluate(() => window.__REVIEW_TEST__.exports)).toBe(2);
    await page.getByRole('button',{ name:'안내 닫기',exact:true }).click();
    await page.getByRole('button',{ name:'개인정보와 학습 안내',exact:true }).click();
    await expect(page.getByRole('heading',{ name:'기기에 남는 음성과 글' })).toBeVisible();
    await finish(page,errors,'first-launch information is revisitable and does not gate export');
  }
  {
    const { page, errors } = await fixture({infoSeen:false,microphone:true,openRecording:false});
    const intro = page.getByRole('region',{name:'기기에 남는 음성과 글'});
    await expect(intro.getByRole('listitem')).toHaveCount(3);
    await expect(intro.getByText('기기 안 학습과 모델 변경분 전송은 준비 중입니다.')).toBeVisible();
    await captureReview(page,'onboarding');
    await intro.getByRole('button',{name:'첫 녹음 시작'}).click();
    await expect(intro).toHaveCount(0);
    await expect.poll(async () => (await readState(page,'microphone')).state).toBe('starting');
    await expect.poll(() => readState(page,'infoSeen')).toBe(true);
    await finish(page,errors,'onboarding opens the native microphone and keeps privacy information accessible');
  }
  {
    const { page, errors } = await fixture({infoSeen:false,openRecording:false,phone:true});
    const intro = page.getByRole('region',{name:'기기에 남는 음성과 글'});
    await expect(intro.getByRole('button',{name:'첫 녹음 시작'})).toHaveCount(0);
    await page.evaluate(() => {window.__REVIEW_TEST__.nextImport = '첫 음성.wav';});
    await intro.getByRole('button',{name:'첫 음성 파일 가져오기'}).tap();
    await expect(page.getByRole('button',{name:'첫 음성.wav',exact:true})).toHaveAttribute('aria-pressed','true');
    await expect(intro).toHaveCount(0);
    await expect.poll(() => readState(page,'infoSeen')).toBe(true);
    await finish(page,errors,'mobile onboarding imports from the file picker without offering an unavailable microphone');
  }
  {
    const { page, errors } = await fixture();
    await startBinary(page);
    await expect(page.locator('textarea')).toHaveCount(0);
    const yes = page.getByRole('button',{ name:'Yes, 그대로 맞음 (1)',exact:true });
    await expect(yes).toBeDisabled(); await page.keyboard.press('1');
    expect(await page.evaluate(() => window.__REVIEW_TEST__.saves)).toBe(0);
    await page.keyboard.press('Space'); await expect(yes).toBeEnabled();
    await page.keyboard.down('1'); await page.keyboard.down('1'); await page.keyboard.up('1');
    await expect(page.getByText('두 번째 자동 전사문',{ exact:true })).toBeVisible();
    await expect(yes).toBeDisabled(); expect(await page.evaluate(() => window.__REVIEW_TEST__.saves)).toBe(1);
    await page.keyboard.press('Space'); await expect(yes).toBeEnabled(); await page.getByRole('button',{ name:'No, 다름',exact:true }).click();
    await expect(page.getByText('세 번째 자동 전사문',{ exact:true })).toBeVisible();
    await page.keyboard.press('Escape');
    const saved = await page.evaluate(() => window.__REVIEW_TEST__.segments);
    expect(saved[0].verified.source).toBe('accepted'); expect(saved[1].verdict).toBe('no'); expect(saved[1].verified).toBeNull(); expect(saved[2].verdict).toBeNull();
    await page.getByRole('button',{ name:'오류와 수정본 교정 목록',exact:true }).click();
    await expect(page.getByText('두 번째 자동 전사문',{ exact:true })).toBeVisible();
    await page.getByLabel('수정 초안').fill('사람이 들은 그대로의 수정본');
    await page.getByRole('button',{ name:'초안 저장',exact:true }).click();
    const confirm = page.getByRole('button',{ name:'이대로 교정 확정 (Ctrl/Command+Enter)',exact:true });
    await expect(confirm).toBeDisabled();
    expect((await page.evaluate(() => window.__REVIEW_TEST__.segments))[1].verified).toBeNull();
    await page.getByRole('button',{ name:'구간 재생 (Space)',exact:true }).click(); await expect(confirm).toBeEnabled();
    await page.keyboard.press('Control+Enter'); await expect(page.getByText('이 목록에 남은 조각이 없습니다. 나가서 저장한 글을 사용할 수 있습니다.')).toBeVisible();
    expect((await page.evaluate(() => window.__REVIEW_TEST__.segments))[1].verified.source).toBe('corrected');
    await finish(page,errors,'explicit Yes/No, held-key guard, separate draft and exact-revision correction confirmation');
  }
  {
    const { page, errors } = await fixture();
    await startBinary(page);
    const yes = page.getByRole('button',{ name:'Yes, 그대로 맞음 (1)',exact:true });
    await page.keyboard.press('Space'); await expect(yes).toBeEnabled();
    await page.evaluate(() => { window.__REVIEW_TEST__.failNextSave = true; }); await yes.click();
    await expect(page.getByRole('alert')).toHaveText('시험 저장 실패');
    await expect(page.getByText('첫 번째 자동 전사문',{exact:true})).toBeVisible();
    await page.keyboard.press('Space'); await expect(yes).toBeEnabled();
    await page.evaluate(() => { window.__REVIEW_TEST__.delayNextSave = 250; }); await yes.click();
    await page.keyboard.press('Escape');
    await expect.poll(() => page.evaluate(() => window.__REVIEW_TEST__.saves)).toBe(1);
    await expect(page.getByRole('heading',{name:'녹음 보관함'})).toBeVisible();
    await startBinary(page);
    await expect(page.getByText('두 번째 자동 전사문',{exact:true})).toBeVisible();
    await expect(yes).toBeDisabled();
    await finish(page,errors,'failed write stays on frame; late success persists without reopening the exited pass');
  }
  {
    const { page, errors } = await fixture({ invalidFirst:true });
    await startBinary(page);
    await page.keyboard.press('Space');
    await expect(page.getByRole('alert')).toHaveText('재생 구간이 비었습니다');
    await page.keyboard.press('n');
    await expect(page.getByText('두 번째 자동 전사문',{exact:true})).toBeVisible();
    const yes = page.getByRole('button',{ name:'Yes, 그대로 맞음 (1)',exact:true });
    await page.keyboard.press('Space'); await expect(yes).toBeEnabled(); await page.keyboard.press('1');
    const saved = await page.evaluate(() => window.__REVIEW_TEST__.segments);
    expect(saved[0].verdict).toBeNull(); expect(saved[0].verified).toBeNull(); expect(saved[1].verified.source).toBe('accepted');
    await page.keyboard.press('Escape');
    await page.getByRole('button',{ name:'텍스트 파일로 저장',exact:true }).click();
    expect(await page.evaluate(() => window.__REVIEW_TEST__.exports)).toBe(1);
    await finish(page,errors,'invalid first audio can be skipped by keyboard without a verdict and later audio remains reachable');
  }
  {
    const { page, errors } = await fixture();
    await startBinary(page);
    await page.keyboard.press('Space');
    await expect(page.getByRole('button',{name:'No, 다름',exact:true})).toBeEnabled();
    await page.getByRole('button',{ name:'No, 다름',exact:true }).click(); await page.keyboard.press('Escape');
    await page.getByRole('button',{name:'오류와 수정본 교정 목록',exact:true}).click();
    const confirm=page.getByRole('button',{name:'이대로 교정 확정 (Ctrl/Command+Enter)',exact:true});
    await page.getByRole('button',{name:'구간 재생 (Space)',exact:true}).click();
    await expect(confirm).toBeDisabled();
    await page.getByLabel('수정 초안').fill('한 번 듣고 고친 말 그대로');
    await expect(confirm).toBeEnabled();
    await confirm.click();
    await expect(page.getByText('이 목록에 남은 조각이 없습니다. 나가서 저장한 글을 사용할 수 있습니다.')).toBeVisible();
    const saved=await page.evaluate(() => window.__REVIEW_TEST__.segments.find(s=>s.id==='s1'));
    expect(saved.text).toBe('한 번 듣고 고친 말 그대로'); expect(saved.verified.source).toBe('corrected'); expect(saved.verified.revision).toBe(saved.revision);
    await page.keyboard.press('Escape'); await expect(page.getByRole('heading',{name:'녹음 보관함'})).toBeVisible();
    await finish(page,errors,'one completed playback permits an atomic edited-text confirmation and completed pass exits with Escape');
  }
  {
    const { page,errors }=await fixture();
    await startBinary(page);
    await page.keyboard.press('Space'); await expect(page.getByRole('button',{name:'No, 다름',exact:true})).toBeEnabled();
    await page.getByRole('button',{ name:'No, 다름',exact:true }).click(); await page.keyboard.press('Escape');
    await page.getByRole('button',{name:'오류와 수정본 교정 목록',exact:true}).click();
    const confirm=page.getByRole('button',{name:'이대로 교정 확정 (Ctrl/Command+Enter)',exact:true});
    await page.getByRole('button',{name:'구간 재생 (Space)',exact:true}).click();
    await expect(page.getByText('이 수정본의 음성을 끝까지 들었습니다.',{exact:true})).toBeVisible();
    await expect(confirm).toBeDisabled();
    await page.keyboard.press('1'); await page.keyboard.press('Control+Enter');
    expect(await page.evaluate(()=>window.__REVIEW_TEST__.segments.find(s=>s.id==='s1').verified)).toBeNull();
    await page.getByLabel('수정 초안').fill('  첫 번째 자동 전사문  ');
    await expect(confirm).toBeDisabled();
    await page.getByRole('button',{name:'처음 판정 취소하고 다시 검수',exact:true}).click();
    await expect(page.getByRole('heading',{name:'자동 전사문 검증',exact:true})).toBeVisible();
    const reset=await page.evaluate(()=>window.__REVIEW_TEST__.segments.find(s=>s.id==='s1'));
    expect(reset.verdict).toBeNull(); expect(reset.verified).toBeNull(); expect(reset.needs_correction).toBe(false);
    const yes=page.getByRole('button',{name:'Yes, 그대로 맞음 (1)',exact:true});
    await expect(yes).toBeDisabled(); await page.keyboard.press('Space'); await expect(yes).toBeEnabled(); await page.keyboard.press('1');
    await expect.poll(()=>page.evaluate(()=>window.__REVIEW_TEST__.segments.find(s=>s.id==='s1').verified?.source)).toBe('accepted');
    await finish(page,errors,'unchanged or whitespace-only No cannot confirm; explicit reset requires a new binary playback and Yes');
  }
  {
    const { page, errors } = await fixture({ quick:true, branch:true });
    await startQuick(page);
    await seekAndPause(page,1.6);
    const context = page.locator('.transcript-context');
    await expect(context.getByText('첫 번째 최신전사문',{exact:true})).toBeVisible();
    await expect(context.getByText('0:00.000 ~ 0:01.500',{exact:true})).toBeVisible();
    await expect(page.locator('.scan-timeline .active-token')).toHaveText('자동 전사문');
    await expect(page.locator('.scan-timeline .timeline-playhead')).toHaveAttribute('data-sample','25600');
    await expect(page.locator('.scan-timeline .timeline-line')).toHaveCount(2);
    await expect(page.locator('.scan-timeline .timeline-tick').nth(1)).toHaveText('0:03.200');
    const scanTop = (await page.locator('.scan-timeline').boundingBox()).y;
    await expect(page.getByRole('heading',{name:'교정 대기',exact:true})).toBeVisible();
    await captureReview(page,'listen');
    await seekAndPause(page,2.25);
    const flag = page.getByRole('button',{name:'오류 표시 (Space)',exact:true});
    await flag.focus();
    await page.keyboard.down('Space'); await page.keyboard.down('Space'); await page.keyboard.up('Space');
    const timeline = page.locator('.sample-timeline');
    await expect(timeline).toHaveAttribute('data-start','4000');
    await expect(timeline).toHaveAttribute('data-end','36000');
    await expect(bar(page,'candidate',4000,20000)).toBeVisible();
    const fallback = timeline.locator('.line-time');
    await expect(fallback).toHaveText('첫 번째 최신전사문');
    await expect(fallback).toHaveAttribute('data-start','0');
    const timed = timeline.locator('.token-time');
    await expect(timed).toHaveCount(2);
    await expect(timed.filter({hasText:'두 번째'})).toHaveAttribute('data-start','20000');
    await expect(timed.filter({hasText:'자동 전사문'})).toHaveAttribute('data-start','22400');
    for (const width of [1280, 560]) {
      await page.setViewportSize({width,height:900});
      expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(width);
      const coordinates = await timeline.evaluate(node => {
        const area = node.getBoundingClientRect();
        const box = selector => node.querySelector(selector).getBoundingClientRect();
        return [
          (box('.token-time[data-start="20000"]').left-area.left)/area.width,
          (box('.token-time[data-start="22400"]').left-area.left)/area.width,
          (box('.timeline-bar.candidate').right-area.left)/area.width,
          box('canvas').width/area.width,
        ];
      });
      [0.5,0.575,0.5,1].forEach((expected,i) => expect(coordinates[i]).toBeCloseTo(expected,3));
    }
    await page.setViewportSize({width:1280,height:900});
    await expect(page.locator('.timeline-context').getByText('두 번째 자동 전사문')).toBeVisible();

    const canvas = timeline.getByRole('img',{name:/원본 음성 스펙트로그램/});
    await expect.poll(() => canvas.evaluate(node => {
      const context = node.getContext('2d');
      const pixel = (time, lowToHighBin) => context.getImageData(Math.floor(node.width*time),node.height-1-lowToHighBin,1,1).data[0];
      return [pixel(0.2,128), pixel(0.55,128)>200, pixel(0.55,32)];
    })).toEqual([18,true,18]);
    await captureReview(page,'narrow');
    await page.getByRole('button',{name:'뒤 절반 듣기',exact:true}).click();
    await expect.poll(async () => (await readState(page,'playCalls')).at(-1)).toEqual({start:20000,end:36000});
    await seekAndPause(page,0.125);
    await expect(timeline.locator('.timeline-playhead')).toHaveAttribute('data-sample','22000');
    await expect(page.locator('audio,video')).toHaveCount(0);
    expect((await readState(page,'output')).position).toBe(22000);

    const chooseRight = page.getByRole('button',{name:'뒤 절반 선택 (→)',exact:true});
    await chooseRight.click();
    await expect(bar(page,'candidate',20000,36000)).toBeVisible();
    expect(await clipRanges(page,'locked')).toEqual([]);
    await chooseRight.focus();
    await page.keyboard.down('Enter'); await page.keyboard.down('Enter'); await page.keyboard.up('Enter');
    await expect(bar(page,'locked',20000,36000)).toBeVisible();
    await expect(bar(page,'candidate',20000,28000)).toBeVisible();
    await page.keyboard.press('Enter');
    await expect(bar(page,'locked',20000,28000)).toBeVisible();
    await expect(bar(page,'candidate',20000,24000)).toBeVisible();
    await page.keyboard.press('ArrowUp'); await page.keyboard.press('ArrowUp');
    expect(await clipRanges(page,'retained')).toEqual([[20000,24000]]);
    expect(await clipRanges(page,'locked')).toEqual([[20000,36000],[20000,28000]]);
    await expect(bar(page,'candidate',4000,20000)).toBeVisible();
    await page.keyboard.press('Enter');
    await expect(bar(page,'candidate',4000,12000)).toBeVisible();
    expect(await readState(page,'markCalls')).toEqual([]);
    expect(await readState(page,'endedCalls')).toBe(0);
    expect(await readState(page,'reviewPlaybackCalls')).toBe(0);

    await page.evaluate(() => { window.__REVIEW_TEST__.lostMarkReply = true; });
    await page.keyboard.down('Escape'); await page.keyboard.down('Escape'); await page.keyboard.up('Escape');
    await expect(page.getByRole('alert')).toHaveText('시험 응답 유실');
    expect(await readState(page,'markCalls')).toHaveLength(1);
    await expect(chooseRight).toBeDisabled();
    expect(await clipRanges(page,'retained')).toEqual([[20000,24000]]);
    await expect(bar(page,'candidate',4000,12000)).toBeVisible();
    await page.keyboard.press('Escape');
    await expect.poll(async () => (await readState(page,'playCalls')).at(-1)?.start).toBe(36000);
    const attempts = await readState(page,'markCalls');
    expect(attempts).toHaveLength(2); expect(attempts[0]).toEqual(attempts[1]);
    expect(attempts[0].ranges).toEqual([{start:20000,end:24000},{start:4000,end:12000}]);
    expect(await readState(page,'marks')).toHaveLength(2);
    const marked = await readState(page,'segments');
    expect(marked.map(s => [s.error_marked_text,s.verified,s.verdict])).toEqual([['첫 번째 최신전사문',null,null],['두 번째 자동 전사문',null,null],[undefined,null,null]]);

    const queue = page.getByRole('complementary',{name:'교정 대기'});
    await expect(queue.getByRole('listitem')).toHaveCount(2);
    await expect(page.locator('.scan-timeline .timeline-bar.retained')).toHaveCount(2);
    expect((await page.locator('.scan-timeline').boundingBox()).y).toBeCloseTo(scanTop, 0);
    const openQueue = queue.getByRole('button',{name:'교정 목록 열기'});
    await expect(openQueue).toBeEnabled();
    await captureReview(page,'pending');
    await openQueue.focus();
    await page.keyboard.press('Enter');
    await expect(page.getByRole('heading',{name:'수정본 그대로 확인',exact:true})).toBeVisible();
    const correction = page.locator('.correction-timeline');
    await expect(correction).toHaveAttribute('data-start','0');
    await expect(correction).toHaveAttribute('data-end','24000');
    await expect(correction.locator('.retained')).toHaveCount(2);
    await expect(page.getByRole('region',{name:'자동 전사 원문'})).toHaveText(/첫 번째 자동 전사문/);
    await expect(page.getByLabel('수정 초안')).toHaveValue('첫 번째 최신전사문');
    await captureReview(page,'correction-ready');
    await page.keyboard.press('Escape');
    await expect(page.getByRole('heading',{name:'표시한 오류 구간',exact:true})).toBeVisible();
    await page.getByRole('button',{name:'오류와 수정본 교정 목록',exact:true}).click();
    await expect(page.getByText('표시한 오류 구간 0:00.250 ~ 0:00.750, 0:01.250 ~ 0:01.500',{exact:true})).toBeVisible();
    const confirm = page.getByRole('button',{name:'이대로 교정 확정 (Ctrl/Command+Enter)',exact:true});
    await page.evaluate(() => { window.__REVIEW_TEST__.autoOutput = false; });
    await page.getByRole('button',{name:'구간 재생 (Space)',exact:true}).click();
    await expect(page.locator('audio[aria-label="현재 조각의 음성"]')).toHaveCount(0);
    await expect(page.getByText('이 수정본의 음성을 끝까지 들었습니다.',{exact:true})).toHaveCount(0);
    await expect(confirm).toBeDisabled();
    await page.evaluate(() => {
      window.__REVIEW_TEST__.advanceOutput(24000);
      window.__REVIEW_TEST__.autoOutput = true;
    });
    await expect(page.getByText('이 수정본의 음성을 끝까지 들었습니다.',{exact:true})).toBeVisible();
    await expect(confirm).toBeDisabled();
    await page.getByLabel('수정 초안').fill('  첫 번째 최신전사문  ');
    await expect(confirm).toBeDisabled();
    await page.keyboard.press('Control+Enter');
    expect(await readState(page,'reviewCalls')).toBe(0);
    await page.getByRole('button',{name:'초안 저장',exact:true}).click();
    await expect.poll(() => readState(page,'saves')).toBe(1);
    await page.keyboard.press('Escape');
    await page.getByRole('button',{name:'오류와 수정본 교정 목록',exact:true}).click();
    await page.getByRole('button',{name:'구간 재생 (Space)',exact:true}).click();
    await expect(page.getByText('이 수정본의 음성을 끝까지 들었습니다.',{exact:true})).toBeVisible();
    await expect(confirm).toBeDisabled();
    await page.keyboard.press('Control+Enter');
    expect(await readState(page,'reviewCalls')).toBe(1);
    await page.getByLabel('수정 초안').fill('첫 번째 최신 전사문');
    await captureReview(page,'correction-edit');
    await expect(confirm).toBeEnabled(); await confirm.click();
    await expect.poll(async () => (await readState(page,'segments'))[0].verified?.text).toBe('첫 번째 최신 전사문');
    await expect(page.getByText('두 번째 자동 전사문',{exact:true})).toBeVisible();
    await expect(page.getByText('표시한 오류 구간 0:01.250 ~ 0:01.500',{exact:true})).toBeVisible();
    await expect(confirm).toBeDisabled();
    await expect(page.getByText('교정을 확정했습니다.',{exact:true})).toBeVisible();
    await expect(page.getByRole('complementary',{name:'교정할 조각'}).getByRole('listitem')).toHaveCount(1);
    await captureReview(page,'correction-next');
    await page.getByRole('button',{name:'이 조각의 오류 판정 해제',exact:true}).click();
    await expect(page.getByText(/이 목록에 남은 조각이 없습니다/)).toBeVisible();
    const dismissed = (await readState(page,'segments'))[1];
    expect(dismissed.error_marked_text).toBeUndefined();
    expect(dismissed.text).toBe('두 번째 자동 전사문');
    expect(dismissed.verified).toBeNull();
    expect(await readState(page,'marks')).toHaveLength(2);
    await finish(page,errors,'PCM time axis, keyboard selection of 0–1 and 4, retained ancestors, idempotent batch and separate full correction');
  }
  {
    const { page, errors } = await fixture({ quick:true, long:true });
    await startQuick(page);
    await expect.poll(async () => (await readState(page,'playCalls')).length).toBe(1);
    await page.evaluate(() => {
      const state = window.__REVIEW_TEST__;
      state.advanceOutput(state.outputRange.end-state.output.position);
    });
    await expect.poll(async () => (await readState(page,'playCalls')).at(-1)).toEqual({start:480000,end:512000});
    await seekAndPause(page,0.2);
    await page.keyboard.press('Space');
    await expect(page.locator('.sample-timeline')).toHaveAttribute('data-start','457600');
    await expect(page.locator('.timeline-line .line-time')).toHaveText('세 번째 자동 전사문');
    await page.evaluate(() => { window.__REVIEW_TEST__.holdNextPlayback = true; });
    await page.getByRole('button',{name:'앞 절반 듣기',exact:true}).click();
    await expect.poll(() => page.evaluate(() => typeof window.__REVIEW_TEST__.releasePlayback)).toBe('function');
    const pendingRange = (await readState(page,'playCalls')).at(-1);
    const staleSession = (await readState(page,'output')).session;
    await page.keyboard.press('Backspace');
    expect((await readState(page,'playCalls')).at(-1)).toEqual(pendingRange);
    await page.evaluate(() => window.__REVIEW_TEST__.releasePlayback());
    await expect.poll(() => readState(page,'delayedPlaybackReturned')).toBe(true);
    await expect.poll(async () => (await readState(page,'controlCalls')).some(call =>
      call.session === staleSession && call.action.kind === 'stop')).toBe(true);
    await expect.poll(async () => (await readState(page,'playCalls')).at(-1)?.start).toBe(483200);
    await seekAndPause(page,0.25);
    await page.keyboard.press('Space');
    await expect(bar(page,'candidate',483200,487200)).toBeVisible();
    await expect(page.getByText(/현재 범위가 마지막 후보입니다/)).toBeVisible();
    expect(await readState(page,'markCalls')).toEqual([]);
    await page.keyboard.press('Enter');
    await expect(bar(page,'locked',483200,487200)).toBeVisible();
    expect(await readState(page,'markCalls')).toEqual([]);
    await page.keyboard.press('Escape');
    await expect.poll(async () => (await readState(page,'playCalls')).at(-1)?.start).toBe(487200);
    expect((await readState(page,'markCalls'))[0].ranges).toEqual([{start:483200,end:487200}]);
    await seekAndPause(page,0.2);
    await page.keyboard.press('n');
    await expect.poll(async () => (await readState(page,'playCalls')).at(-1)).toEqual({start:505600,end:512000});
    await expect(page.getByText('녹음 끝',{exact:true})).toBeVisible();
    expect(await readState(page,'endedCalls')).toBe(0);
    expect(await readState(page,'reviewPlaybackCalls')).toBe(0);
    expect(await readState(page,'markCalls')).toHaveLength(1);
    expect((await readState(page,'segments')).every(s => !s.verified && !s.verdict)).toBe(true);
    await finish(page,errors,'chunk transition, stale preview cancellation and explicit selection at minimum size');
  }
  {
    const {page,errors} = await fixture({quick:true,long:true});
    await startQuick(page);
    await expect.poll(async () => (await readState(page,'playCalls')).length).toBe(1);
    await page.evaluate(() => {
      const state = window.__REVIEW_TEST__;
      state.holdNextPlayback = true;
      state.advanceOutput(state.outputRange.end-state.output.position);
    });
    await expect.poll(() => page.evaluate(() => typeof window.__REVIEW_TEST__.releasePlayback)).toBe('function');
    const stale = (await readState(page,'output')).session;
    await page.keyboard.press('Space');
    await expect(page.locator('.sample-timeline')).toHaveAttribute('data-start','457600');
    await expect(page.locator('.sample-timeline')).toHaveAttribute('data-end','480000');
    await page.evaluate(() => window.__REVIEW_TEST__.releasePlayback());
    await expect.poll(async () => (await readState(page,'controlCalls')).some(call =>
      call.session === stale && call.action.kind === 'stop')).toBe(true);
    expect((await readState(page,'playCalls')).length).toBe(2);
    await finish(page,errors,'Space during preparation marks the confirmed chunk end and stops the late preview');
  }
  {
    const {page,errors} = await fixture({quick:true});
    await page.evaluate(() => { window.__REVIEW_TEST__.holdNextPlayback = true; });
    await startQuick(page);
    await expect.poll(() => page.evaluate(() => typeof window.__REVIEW_TEST__.releasePlayback)).toBe('function');
    const stale = (await readState(page,'output')).session;
    await page.getByRole('button',{name:'검수 나가기',exact:true}).click();
    await startBinary(page);
    const play = page.getByRole('button',{name:'구간 재생 (Space)',exact:true});
    await expect(play).toBeDisabled();
    await page.keyboard.press('Space');
    expect(await readState(page,'reviewPlaybackCalls')).toBe(0);
    await page.evaluate(() => {
      window.__REVIEW_TEST__.holdNextStop = true;
      window.__REVIEW_TEST__.releasePlayback();
    });
    await expect.poll(() => page.evaluate(() => typeof window.__REVIEW_TEST__.releaseStop)).toBe('function');
    await expect(play).toBeDisabled();
    await page.keyboard.press('Space');
    expect(await readState(page,'reviewPlaybackCalls')).toBe(0);
    await page.evaluate(() => window.__REVIEW_TEST__.releaseStop());
    await expect.poll(async () => (await readState(page,'controlCalls')).some(call =>
      call.session === stale && call.action.kind === 'stop')).toBe(true);
    await expect(play).toBeEnabled();
    await page.keyboard.press('Space');
    await expect.poll(() => readState(page,'reviewPlaybackCalls')).toBe(1);
    await expect(page.getByRole('button',{name:'Yes, 그대로 맞음 (1)',exact:true})).toBeEnabled();
    expect((await readState(page,'playCalls')).length).toBe(1);
    await finish(page,errors,'other-screen playback waits for the late preview and its stop to finish');
  }
  {
    const { page, errors } = await fixture({quick:true,branch:true});
    await startQuick(page);
    await seekAndPause(page,2.25);
    const oldSession = (await readState(page,'output')).session;
    await page.evaluate(() => {
      Object.assign(window.__REVIEW_TEST__.output,{state:'failed',heard:false,error:'시험 출력 장치 오류'});
    });
    await expect(page.getByRole('alert')).toHaveText('시험 출력 장치 오류');
    await page.getByRole('button',{name:'재생 다시 시도',exact:true}).click();
    await expect.poll(async () => (await readState(page,'playCalls')).at(-1).start).toBe(36000);
    const session = (await readState(page,'output')).session;
    expect(session).not.toBe(oldSession);
    await page.getByRole('button',{name:'검수 나가기',exact:true}).click();
    await expect.poll(async () => (await readState(page,'controlCalls')).some(call =>
      call.session === session && call.action.kind === 'stop')).toBe(true);
    await expect(page.locator('audio,video')).toHaveCount(0);
    expect(await readState(page,'reviewPlaybackCalls')).toBe(0);
    expect(await readState(page,'reviewCalls')).toBe(0);
    await finish(page,errors,'runtime device error permits retry at the last confirmed source position and exit stops only that preview session');
  }
  {
    const { page, errors } = await fixture({ quick:true, branch:true });
    await startQuick(page);
    await seekAndPause(page,2.25);
    await page.keyboard.press('Space');
    await page.keyboard.press('ArrowRight');
    await page.keyboard.press('Enter');
    await page.keyboard.press('Enter');
    await page.keyboard.press('ArrowUp');
    await page.keyboard.press('ArrowUp');
    await page.keyboard.press('Enter');
    await expect(bar(page,'locked',4000,20000)).toBeVisible();
    await page.keyboard.press('Backspace');
    await expect(bar(page,'locked',4000,20000)).toHaveCount(0);
    await page.keyboard.press('Backspace');
    expect(await clipRanges(page,'candidate')).toEqual([]);
    expect(await clipRanges(page,'retained')).toEqual([[20000,24000]]);
    expect(await readState(page,'markCalls')).toEqual([]);
    await page.keyboard.press('Escape');
    await expect.poll(() => readState(page,'markCalls')).toHaveLength(1);
    expect((await readState(page,'markCalls'))[0].ranges).toEqual([{start:20000,end:24000}]);
    await finish(page,errors,'Backspace at root discards the active candidate and Escape keeps the retained branch');
  }
  {
    const { page, errors } = await fixture({ quick:true, branch:true });
    await startQuick(page);
    await seekAndPause(page,2.25);
    await page.keyboard.press('Space');
    await expect(bar(page,'candidate',4000,20000)).toBeVisible();
    await page.keyboard.press('Escape');
    await expect(page.locator('.candidate-timeline')).toHaveCount(0);
    expect(await readState(page,'markCalls')).toEqual([]);

    // Restart at sample zero so the same original candidate is available again.
    await page.keyboard.press('Escape');
    await startQuick(page);
    await seekAndPause(page,2.25);
    await page.keyboard.press('Space');
    await page.keyboard.press('ArrowRight');
    await page.keyboard.press('Enter');
    await page.keyboard.press('Enter');
    await page.keyboard.press('ArrowUp');
    await page.keyboard.press('ArrowUp');
    await expect(bar(page,'candidate',4000,20000)).toBeVisible();
    await page.keyboard.press('Escape');
    await expect.poll(() => readState(page,'markCalls')).toHaveLength(1);
    expect((await readState(page,'markCalls'))[0].ranges).toEqual([{start:20000,end:24000}]);
    await finish(page,errors,'Escape saves only explicitly selected audio when the root shows its default left half');
  }
  {
    const { page, errors } = await fixture({ quick:true, branch:true });
    await startQuick(page);
    await seekAndPause(page,2.25);
    for (const key of ['Space','ArrowRight','Enter','Enter','ArrowUp','ArrowUp','Enter']) {
      await page.keyboard.press(key);
    }
    await page.evaluate(() => { window.__REVIEW_TEST__.failNextMark = true; });
    await page.keyboard.press('Escape');
    await expect(page.getByRole('alert')).toHaveText('시험 오류 구간 저장 실패');
    expect(await clipRanges(page,'retained')).toEqual([[20000,24000]]);
    await page.keyboard.press('Backspace');
    await expect(bar(page,'candidate',4000,20000)).toBeVisible();
    await page.keyboard.press('Enter');
    await page.keyboard.press('ArrowRight');
    await expect(bar(page,'candidate',12000,20000)).toBeVisible();
    await page.keyboard.press('Escape');
    await expect.poll(() => readState(page,'markCalls')).toHaveLength(2);
    const [failed, edited] = await readState(page,'markCalls');
    expect(edited.id).not.toEqual(failed.id);
    expect(edited.ranges).toEqual([{start:20000,end:24000},{start:12000,end:20000}]);
    expect(await readState(page,'marks')).toHaveLength(2);
    await finish(page,errors,'Backspace after a failed write keeps retained branches and allows a different request');
  }
  {
    const { page, errors } = await fixture({quick:true,branch:true,phone:true});
    const tap = name => page.getByRole('button',{name,exact:true}).tap();
    await tap('빠른 검수 시작');
    await seekAndPause(page,2.25);
    await expect(page.getByRole('button',{name:'오류 표시',exact:true})).toBeVisible();
    await expect(page.getByRole('button',{name:'오류 표시 (Space)',exact:true})).toHaveCount(0);
    await captureReview(page,'mobile-listen');
    await tap('오류 표시');
    await expect(page.getByRole('button',{name:'보관 후 올라가기'})).toBeDisabled();
    await tap('이어듣기');
    expect(await readState(page,'markCalls')).toEqual([]);
    await tap('검수 나가기');
    await tap('빠른 검수 시작');
    await seekAndPause(page,2.25);
    await tap('오류 표시');
    await tap('뒤 절반 듣기');
    await expect.poll(async () => (await readState(page,'playCalls')).at(-1)).toEqual({start:20000,end:36000});
    await tap('뒤 절반 선택');
    await tap('더 좁히기');
    await tap('더 좁히기');
    await tap('보관 후 올라가기');
    await tap('보관 후 올라가기');
    await tap('더 좁히기');
    await tap('뒤 절반 선택');
    await tap('선택 취소');
    await tap('더 좁히기');
    await expect(bar(page,'candidate',4000,12000)).toBeVisible();
    await expect(bar(page,'retained',20000,24000)).toBeVisible();
    await captureReview(page,'mobile-narrow');
    await page.evaluate(() => { window.__REVIEW_TEST__.lostMarkReply = true; });
    await tap('저장 후 이어듣기');
    await tap('저장 다시 시도');
    const attempts = await readState(page,'markCalls');
    expect(attempts).toHaveLength(2);
    expect(attempts[0]).toEqual(attempts[1]);
    expect(attempts[0].ranges).toEqual([{start:20000,end:24000},{start:4000,end:12000}]);
    const queue = page.getByRole('complementary',{name:'교정 대기'});
    await expect(queue.getByRole('listitem')).toHaveCount(2);
    await expect(queue.getByText('오류 2개')).toBeVisible();
    await expect(page.locator('.scan-timeline .timeline-bar.retained')).toHaveCount(2);
    expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(390);
    await captureReview(page,'mobile-pending');
    await tap('교정 목록 열기');
    await expect(page.getByRole('heading',{name:'수정본 그대로 확인',exact:true})).toBeVisible();
    await finish(page,errors,'phone touch review compares both halves, retains branches, cancels a choice and retries a save');
  }
  {
    const page = await browser.newPage({viewport:{width:1280,height:900}});
    const errors = [];
    page.on('pageerror', error => errors.push(error.message));
    page.on('response', response => { if (!response.ok()) errors.push(`${response.status()} ${response.url()}`); });
    await page.goto(`${url}/download/`);
    await expect(page.getByRole('heading',{level:1})).toHaveText(/녹음부터 교정까지/);
    expect(await page.getByRole('img').evaluate(img => img.naturalWidth)).toBeGreaterThan(0);
    await captureReview(page,'download-desktop');
    await page.getByRole('link',{name:'시험 빌드 보기',exact:true}).click();
    await expect(page).toHaveURL(/#download$/);
    await expect(page.getByRole('link',{name:'GitHub 빌드 목록'})).toHaveAttribute('href','https://github.com/guny524/seamone/actions/workflows/build.yml');
    await expect(page.getByText('기기 안 학습과 모델 변경분 전송은 준비 중입니다.')).toBeVisible();
    await page.setViewportSize({width:390,height:844});
    expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(390);
    await finish(page,errors,'download page loads its preview, explains current and planned privacy and links to test builds');
  }
  {
    const { page, errors } = await fixture({openRecording:false});
    let requests = 0;
    const receiver = createServer((_request, response) => {
      requests++;
      response.writeHead(204, { 'Access-Control-Allow-Origin': url }).end();
    });
    try {
      await new Promise((done, reject) => {
        receiver.once('error', reject);
        receiver.listen(0, '127.0.0.1', done);
      });
      const target = `http://127.0.0.1:${receiver.address().port}/csp-probe`;
      const result = await page.evaluate(async target => {
        window.probeViolations = [];
        document.addEventListener('securitypolicyviolation', event => {
          if (event.effectiveDirective === 'connect-src') {
            window.probeViolations.push({ blocked: event.blockedURI, disposition: event.disposition });
          }
        });
        const options = { credentials:'omit', referrerPolicy:'no-referrer', cache:'no-store', signal:AbortSignal.timeout(3000) };
        const local = await fetch('/favicon.ico', options);
        try {
          const foreign = await fetch(target, options);
          return { local:local.status, foreign:foreign.status };
        } catch (error) {
          return { local:local.status, error:error.name };
        }
      }, target);
      expect(result).toEqual({local:204, error:'TypeError'});
      await expect.poll(() => page.evaluate(origin => window.probeViolations.some(event =>
        event.disposition === 'enforce' && new URL(event.blocked).origin === origin
      ), new URL(target).origin)).toBe(true);
      expect(requests).toBe(0);
      await finish(page, errors.filter(message => !message.includes(target)),
        'app CSP permits local fetch and blocks cross-origin fetch before it reaches a server');
    } finally {
      if (!page.isClosed()) await page.close();
      if (receiver.listening) await new Promise(done => receiver.close(done));
    }
  }
  console.log(`${completed} browser scenarios passed`);
} finally { await browser.close(); await new Promise(done => server.close(done)); }
