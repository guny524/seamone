import assert from 'node:assert/strict';
import {test} from 'node:test';
import {installReviewFixture} from './fixture.mjs';

function fixture() {
  globalThis.window = {};
  installReviewFixture({wav:Buffer.alloc(44).toString('base64'),infoSeen:true});
  const invoke = window.__TAURI__.core.invoke;
  return {state:window.__REVIEW_TEST__,invoke};
}
const args = {recordingId:'r',revision:1,range:{start:0,end:6400}};

test('play and pause replies precede callback processing and preserve confirmed positions',async () => {
  const {state,invoke} = fixture();
  const prepared = await invoke('play_recording',args);
  assert.equal(prepared.state,'ready');
  assert.equal(prepared.position,0);
  const playing = await invoke('get_playback_status',{});
  assert.equal(playing.state,'playing');
  assert.equal(playing.position,640);
  const paused = await invoke('control_playback',{session:prepared.session,action:{kind:'pause'}});
  assert.equal(paused.state,'playing');
  assert.equal(paused.position,640);
  const after = await invoke('get_playback_status',{});
  assert.equal(after.state,'paused');
  assert.equal(after.position,640);
  assert.equal(state.outputCommands.length,0);
});

test('seek is queued and stop cancels queued output without a future callback',async () => {
  const {invoke} = fixture();
  const {session} = await invoke('play_recording',args);
  await invoke('get_playback_status',{});
  const pending = await invoke('control_playback',{session,action:{kind:'seek',at:3200}});
  assert.equal(pending.position,640);
  await invoke('control_playback',{session,action:{kind:'pause'}});
  assert.equal((await invoke('get_playback_status',{})).position,3200);
  await invoke('control_playback',{session,action:{kind:'play'}});
  await invoke('control_playback',{session,action:{kind:'stop'}});
  const stopped = await invoke('get_playback_status',{});
  assert.equal(stopped.state,'paused');
  assert.equal(stopped.position,3200);
});

test('late preview response can be stopped by its exact session before another starts',async () => {
  const {state,invoke} = fixture();
  state.holdNextPlayback = true;
  const late = invoke('play_recording',args);
  const oldSession = state.output.session;
  state.releasePlayback();
  assert.equal((await late).session,oldSession);
  state.holdNextStop = true;
  const stopping = invoke('control_playback',{session:oldSession,action:{kind:'stop'}});
  assert.equal(typeof state.releaseStop,'function');
  state.releaseStop();
  await stopping;
  assert.ok(state.controlCalls.some(call => call.session===oldSession && call.action.kind==='stop'));
  const next = await invoke('play_recording',args);
  await assert.rejects(invoke('control_playback',{session:oldSession,action:{kind:'stop'}}));
  assert.equal((await invoke('get_playback_status',{})).session,next.session);
});

test('a fully output preview remains unable to authorize review',async () => {
  const {state,invoke} = fixture();
  const preview = await invoke('play_recording',args);
  state.advanceOutput(6400);
  assert.equal(state.output.heard,true);
  await assert.rejects(invoke('save_review',{proof:preview.session,request:{event_id:'attempt',
    segment_id:'s1',expected_revision:1,action:{kind:'judge',verdict:'yes'}}}));
});
