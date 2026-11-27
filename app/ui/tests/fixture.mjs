export const installReviewFixture = ({ wav, infoSeen, invalidFirst, quick, long, branch, microphone }) => {
    const recording = { id: 'r', name: '시험 녹음.wav', samples: long ? 512000 : quick ? 102400 : 19200, revision: 1, engine: 'test', model: 'fixture' };
    const segments = ['첫 번째 자동 전사문', '두 번째 자동 전사문', '세 번째 자동 전사문'].map((text, i) => ({
      id: `s${i + 1}`, recording_id: 'r', recording_revision: 1, revision: 1, range: { start: i * 6400, end: (i + 1) * 6400 },
      machine_text: text, text, verdict: null, needs_correction: false, verified: null,
    }));
    if (invalidFirst) segments[0].range.end = 0;
    if (quick) {
      segments[0].range = {start:0,end:24000};
      segments[0].text = '첫 번째 최신전사문';
      segments[0].needs_correction = true;
      segments[1].range = {start:27200,end:52800};
      segments[2].range = long ? {start:473600,end:505600} : {start:64000,end:80000};
    }
    if (branch) {
      // The latest line starts 20000 samples in, so the 1-second lookback starts at 4000.
      segments[1].range.start = 20000;
      segments[1].machine_timing = [
        { text:'두 번째 ', range:{start:20000,end:22400} },
        { text:'자동 전사문', range:{start:22400,end:52800} },
      ];
      // The edited first line must never use the machine's token locations.
      segments[0].machine_timing = [
        { text:segments[0].machine_text, range:{start:1600,end:23000} },
      ];
    }
    const proofs = new Map(); const events = new Map(); const batches = new Map();
    const state = { segments, recordings: [recording], saves: 0, delayNextSave: 0, failNextSave: false, infoSeen, exports: 0,
      microphone: { state: microphone ? 'idle' : 'unavailable' }, finishCalls: [], failMicSave: false, nextImport: null,
      marks: [], markCalls: [], rangeCalls: [], playCalls: [], controlCalls: [], reviewCalls: 0, reviewPlaybackCalls: 0, endedCalls: 0, holdNextPlayback: false, holdNextStop: false, failNextMark: false, lostMarkReply: false };
    state.output = {session:null, state:'idle', position:0, heard:false, error:null};
    state.autoOutput = true;
    state.outputCommands = [];
    state.processOutput = () => {
      for (const action of state.outputCommands.splice(0)) {
        const proof = proofs.get(state.output.session);
        if (!proof || state.output.state === 'failed') continue;
        if (action.kind === 'play') {
          if (state.output.position === proof.end) state.output.position = proof.start;
          state.output.state = 'playing';
        } else if (action.kind === 'pause') state.output.state = 'paused';
        else if (action.kind === 'seek') {
          state.output.position = action.at;
          if (state.output.state === 'finished') state.output.state = 'paused';
        }
      }
    };
    state.advanceOutput = frames => {
      state.processOutput();
      const output = state.output, proof = proofs.get(output.session);
      if (output.state !== 'playing' || !proof) return;
      const start = output.position, end = Math.min(proof.end,start + frames);
      proof.ranges.push([start,end]);
      const merged = [];
      for (const range of proof.ranges.toSorted((a,b) => a[0]-b[0])) {
        const last = merged.at(-1);
        if (last && range[0] <= last[1]) last[1] = Math.max(last[1],range[1]);
        else merged.push([...range]);
      }
      proof.ranges = merged;
      proof.complete = merged.length === 1 && merged[0][0] === proof.start && merged[0][1] === proof.end;
      output.position = end; output.heard = proof.complete;
      if (end === proof.end) output.state = 'finished';
    };
    const prepareOutput = (id,revision,range) => {
      proofs.clear();
      state.outputCommands.length = 0;
      const session = crypto.randomUUID();
      proofs.set(session,{id,revision,start:range.start,end:range.end,ranges:[],complete:false});
      state.outputRange = {...range};
      state.output = {session,state:'ready',position:range.start,heard:false,error:null};
      return session;
    };
    window.__REVIEW_TEST__ = state;
    const copy = value => JSON.parse(JSON.stringify(value));
    const rangeWav = range => {
      const wave = new Uint8Array(44 + (range.end - range.start) * 2);
      wave.set(Uint8Array.from(atob(wav.slice(0,60)).slice(0,44), char => char.charCodeAt(0)));
      const header = new DataView(wave.buffer);
      header.setUint32(4, wave.length - 8, true); header.setUint32(40, wave.length - 44, true);
      if (branch) {
        const pattern = [0, 24000, 0, -24000]; // Tone only in unit 4, 4 kHz at 16 kHz.
        for (let i = Math.max(range.start,20000); i < Math.min(range.end,24000); i++) {
          header.setInt16(44 + (i-range.start)*2,pattern[i%4],true);
        }
      }
      let binary = '';
      for (let i = 0; i < wave.length; i += 8192) binary += String.fromCharCode(...wave.subarray(i,i+8192));
      return btoa(binary);
    };
    window.__TAURI__ = { core: { invoke: async (command, args) => {
      if (args instanceof Map) throw 'IPC arguments must be plain JSON objects';
      if (command === 'get_library') return { recordings: copy(state.recordings), info_seen: state.infoSeen, pending_training: segments.filter(s => s.verified).length, retry_training:0, inference_available: false };
      if (command === 'get_segments') return { segments: args.recordingId === 'r' ? copy(segments) : [], next_offset: null };
      if (command === 'get_capture_status') return copy(state.microphone);
      if (command === 'start_recording') {
        if (Object.keys(args).length) throw 'start takes no arguments';
        state.microphone = { state: 'starting', session_id: 'microphone-session' };
        return copy(state.microphone);
      }
      if (command === 'finish_recording') {
        if (args.sessionId !== state.microphone.session_id) throw 'stale microphone session';
        state.finishCalls.push(copy(args));
        await new Promise(resolve => { state.releaseMicFinish = resolve; });
        if (state.failMicSave) {
          state.failMicSave = false;
          state.microphone = { state: 'interrupted', session_id: args.sessionId, elapsed_seconds: 12, reason: '시험 저장 실패' };
          throw '시험 저장 실패';
        }
        state.microphone = { state: 'idle' };
        if (!args.save) return null;
        const saved = { ...recording, id: 'captured', name: '마이크 녹음.wav' };
        state.recordings.unshift(saved);
        return saved;
      }
      if (command === 'import_audio') {
        if (Object.keys(args).length) throw 'import takes no path or audio payload';
        const filename = state.nextImport;
        state.nextImport = null;
        if (!filename) return null;
        const saved = { ...recording, id: 'imported', name: filename };
        state.recordings.unshift(saved);
        return saved;
      }
      if (command === 'get_error_marks') return { marks: copy(state.marks), next_offset: null };
      if (command === 'get_recording_playback') {
        state.rangeCalls.push(copy(args.range));
        return rangeWav(args.range);
      }
      if (command === 'save_error_marks') {
        const batch = copy(args.batch);
        state.markCalls.push(batch);
        if (batches.has(batch.id)) {
          if (JSON.stringify(batch) !== batches.get(batch.id)) throw '충돌';
          return;
        }
        if (state.failNextMark) { state.failNextMark = false; throw '시험 오류 구간 저장 실패'; }
        batches.set(batch.id, JSON.stringify(batch));
        batch.ranges.toSorted((a,b) => a.start-b.start).forEach((range, i) => state.marks.push({
          id:`${batch.id}-${i}`, range, recording_id:batch.recording_id,
          recording_revision:batch.recording_revision,
        }));
        for (const s of segments.filter(s => batch.ranges.some(range => s.range.start < range.end && range.start < s.range.end))) {
          s.error_marked_text = s.text; s.verified = null; s.revision++;
        }
        if (state.lostMarkReply) { state.lostMarkReply = false; throw '시험 응답 유실'; }
        return;
      }
      if (command === 'dismiss_info') { state.infoSeen = true; return; }
      if (command === 'export_text') { state.exports++; return segments.map(s => s.text).join('\n'); }
      if (command === 'save_text_file') { state.exports++; return true; }
      if (command === 'copy_saved_text') { state.exports++; return; }
      if (command === 'get_playback') {
        state.reviewPlaybackCalls++;
        const s = segments.find(s => s.id === args.segmentId);
        if (s.revision !== args.revision) throw '다른 수정본이 저장됐습니다';
        if (s.range.start >= s.range.end) throw '재생 구간이 비었습니다';
        const session = prepareOutput(s.id,s.revision,s.range);
        return { segment_id:s.id, revision:s.revision, wav_base64:rangeWav(s.range), session };
      }
      if (command === 'play_recording') {
        state.playCalls.push(copy(args.range));
        prepareOutput(null,args.revision,args.range);
        state.outputCommands.push({kind:'play'});
        const prepared = copy(state.output);
        if (state.holdNextPlayback) {
          state.holdNextPlayback = false;
          await new Promise(resolve => { state.releasePlayback = resolve; });
          state.delayedPlaybackReturned = true;
        }
        return prepared;
      }
      if (command === 'control_playback') {
        state.controlCalls.push(copy(args));
        if (args.session !== state.output.session) throw 'stale playback session';
        const proof = proofs.get(args.session), action = args.action;
        if (action.kind === 'stop') {
          state.outputCommands.length = 0;
          if (state.output.state !== 'failed') state.output.state = 'paused';
          const stopped = copy(state.output);
          if (state.holdNextStop) {
            state.holdNextStop = false;
            await new Promise(resolve => { state.releaseStop = resolve; });
          }
          return stopped;
        } else {
          if (state.output.state === 'failed') throw state.output.error;
          if (!['play','pause','seek'].includes(action.kind)) throw 'unknown playback control';
          if (action.kind === 'seek' && (action.at < proof.start || action.at > proof.end)) throw 'outside playback range';
          state.outputCommands.push(copy(action));
        }
        return copy(state.output);
      }
      if (command === 'get_playback_status') {
        state.advanceOutput(state.autoOutput ? 640 : 0);
        return copy(state.output);
      }
      if (command === 'save_review') {
        state.reviewCalls++;
        const req = args.request;
        const canonical = JSON.stringify(req);
        if (events.has(req.event_id)) {
          const old = events.get(req.event_id); if (old.canonical !== canonical) throw '충돌'; return copy(old.saved);
        }
        const delay = state.delayNextSave; state.delayNextSave = 0;
        if (delay) await new Promise(resolve => setTimeout(resolve,delay));
        if (state.failNextSave) { state.failNextSave = false; throw '시험 저장 실패'; }
        const s = segments.find(s => s.id === req.segment_id);
        if (s.revision !== req.expected_revision) throw '다른 수정본이 저장됐습니다';
        if (['judge','confirm_correction','correct_and_confirm'].includes(req.action.kind)) {
          const p = proofs.get(args.proof);
          if (!p?.complete || p.id !== s.id || p.revision !== s.revision) throw '끝까지 들으세요';
        }
        const confirming=['confirm_correction','correct_and_confirm'].includes(req.action.kind);
        const corrected=req.action.kind==='correct_and_confirm' ? req.action.text : s.text;
        if (confirming && s.verdict==='no' && corrected.trim()===s.machine_text.trim()) throw '처음 판정을 취소하고 다시 검수하세요';
        if (confirming && s.error_marked_text?.trim()===corrected.trim()) throw '오류를 표시할 때 저장된 글과 같습니다';
        if (req.action.kind === 'save_draft' && req.action.text === s.text) return copy(s);
        s.revision++; s.verified = null;
        if (req.action.kind === 'judge') {
          s.verdict = req.action.verdict; s.needs_correction = s.verdict === 'no';
          if (s.verdict === 'yes') s.verified = { revision:s.revision,text:s.text,source:'accepted' };
        } else if (req.action.kind === 'save_draft') { s.text = req.action.text; s.needs_correction = true; }
        else if (req.action.kind === 'correct_and_confirm') { s.text=req.action.text; s.needs_correction = true; s.verified={ revision:s.revision,text:s.text,source:'corrected' }; }
        else if (req.action.kind === 'confirm_correction') s.verified = { revision:s.revision,text:s.text,source:'corrected' };
        else if (req.action.kind === 'dismiss_error_mark') delete s.error_marked_text;
        else if (req.action.kind === 'invalidate') { const unchanged=s.verdict==='no' && s.text.trim()===s.machine_text.trim(); s.verdict=null; if (unchanged) { s.text=s.machine_text; s.needs_correction=false; } else s.needs_correction ||= s.text !== s.machine_text; }
        state.saves++; events.set(req.event_id,{ canonical,saved:copy(s) }); return copy(s);
      }
      throw `Unexpected test command: ${command}`;
    } } };
  };
