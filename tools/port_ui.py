# One-off port of the PicoDJ web build (v0.6.1) to the Tauri app UI.
import sys
src=open(sys.argv[1],encoding='utf-8').read()
def R(a,b,count=1):
    global src
    assert src.count(a)==count, (src.count(a), a[:90])
    src=src.replace(a,b)
# document shell
src='<!doctype html>\n<html lang="vi">\n<head>\n<meta charset="utf-8">\n<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">\n'+src
R('</style>\n','</style>\n</head>\n<body>\n')
src=src.rstrip()+'\n</body>\n</html>\n'
R('<small>PicoDJ v0.6.1</small>','<small>PicoDJ v0.7</small>')
R('__LIBRARY__','{"tracks":[],"playlists":[]}')
# ── native layer: folders + files ──
R("""async function pickFolder(){ ensureAudio();
  if(window.showDirectoryPicker)""","""// ── native app (Tauri): music folders are scanned by Rust, files read straight from disk ──
const TAURI=window.__TAURI__||null, inv=TAURI?TAURI.core.invoke:null;
class DiskFile{ constructor(e){ this.path=e.path; this.name=e.name; this.size=e.size; }
  async arrayBuffer(){ return await inv('read_file',{path:this.path}); }
  slice(a,b){ const p=this.path, n=Math.max(0,Math.min(b??this.size,this.size)-a); return {arrayBuffer:()=>inv('read_range',{path:p,start:a,len:n})}; } }
let DIRS=[]; try{ DIRS=JSON.parse(localStorage.getItem('picodj.dirs')||'[]').map(p=>({path:p,names:[]})); }catch(e){}
function saveDirs(){ try{ localStorage.setItem('picodj.dirs',JSON.stringify(DIRS.map(d=>d.path))); }catch(e){} }
async function scanDir(d){ try{ const list=await inv('scan_folder',{path:d.path}); d.names=[]; for(const e of list){ const k=e.name.toLowerCase(); if(!FILES.has(k)) FILES.set(k,new DiskFile(e)); d.names.push(k); } d.err=''; }catch(e){ d.err=String(e); d.names=[]; log('Thư mục: '+e); } }
async function rescanDirs(){ for(const [k,v] of [...FILES]) if(v instanceof DiskFile) FILES.delete(k); for(const d of DIRS) await scanDir(d); folderReady(); }
const dirName=p=>p.split(/[\\\\/]/).filter(Boolean).slice(-2).join(' / ')||p;
async function pickFolder(){ ensureAudio();
  if(TAURI){ try{ const p=await inv('plugin:dialog|open',{options:{directory:true,multiple:false,title:'Chọn thư mục nhạc'}}); if(!p) return;
      if(DIRS.some(d=>d.path===p)) return toast('Thư mục này đã có trong danh sách');
      const d={path:p,names:[]}; DIRS.push(d); saveDirs(); await scanDir(d); S.open.dir=true; S.list='dir:'+(DIRS.length-1); S.sel=0; folderReady(); toast(`Đã thêm ${d.names.length} file từ ${dirName(p)}`);
    }catch(e){ toast('Không mở được thư mục: '+e); } return; }
  if(window.showDirectoryPicker)""")
R("(async()=>{ await loadPersist(); const h=await idb.get('dir'); if(!h) return;","(async()=>{ await loadPersist(); if(TAURI){ if(DIRS.length) await rescanDirs(); return; } const h=await idb.get('dir'); if(!h) return;")
R("if(!f) return null; return f instanceof File ? f : await f.getFile(); }","if(!f) return null; return f.getFile ? await f.getFile() : f; }")
R("document.getElementById('foldertxt').textContent=`${FILES.size} FILE","document.getElementById('foldertxt').textContent=`${TAURI?`${DIRS.length} THƯ MỤC · `:''}${FILES.size} FILE")
# tree: music folders group
R("""function treeItems(){ const it=[{key:'col',name:'Collection',ids:collectionIds(),depth:0}];""",
"""function treeItems(){ const it=[{key:'col',name:'Collection',ids:collectionIds(),depth:0}];
  if(TAURI){ it.push({key:'grp:dir',name:'Thư mục nhạc',group:'dir',depth:0});
    if(S.open.dir!==false){ DIRS.forEach((d,i)=>it.push({key:'dir:'+i,name:dirName(d.path)+(d.err?' (không mở được)':''),title:d.path,ids:d.names.map(n=>trackForFile(FILES.get(n).name).id),dir:i,depth:1})); it.push({key:'adddir',name:'＋ Thêm thư mục nhạc',add:1,depth:1}); } }""")
R("""${x.user?`<span class="x" data-del="${x.ui}" title="Xoá playlist">✕</span>`:''}</div>`""",
  """${x.user?`<span class="x" data-del="${x.ui}" title="Xoá playlist">✕</span>`:''}${x.dir!==undefined?`<span class="x" data-deldir="${x.dir}" title="Bỏ thư mục khỏi danh sách (không xoá file)">✕</span>`:''}</div>`""")
R("""data-key="${x.key}"><span class="tw">""","""data-key="${x.key}"${x.title?` title="${esc(x.title)}"`:''}><span class="tw">""")
R("""document.getElementById('tree').addEventListener('click',e=>{ const x=e.target.closest('.x');""",
  """document.getElementById('tree').addEventListener('click',e=>{ const xd=e.target.closest('[data-deldir]');
  if(xd){ DIRS.splice(+xd.dataset.deldir,1); saveDirs(); S.list='col'; rescanDirs(); toast('Đã bỏ thư mục khỏi danh sách (file trên máy vẫn còn)'); return; }
  const x=e.target.closest('.x');""")
R("  if(k==='xml'){ document.getElementById('xmlinput').click(); return; }","  if(k==='xml'){ document.getElementById('xmlinput').click(); return; }\n  if(k==='adddir'){ pickFolder(); return; }")
# ── MIDI through Rust (all ports, hot-plug), LED feedback out ──
R("""async function connectMidi(){ ensureAudio();
  if(!navigator.requestMIDIAccess)""","""async function connectMidi(){ ensureAudio();
  if(TAURI){ try{ const names=await inv('midi_connect_all'); pill.classList.toggle('on',names.length>0); mtxt.textContent=names.length?`MIDI · ${names.length} THIẾT BỊ`:'MIDI · CHƯA THẤY BÀN'; pill.title=names.length?names.join('\\n'):'Chưa thấy thiết bị MIDI nào — cắm bàn qua USB'; }catch(e){ mtxt.textContent='MIDI LỖI'; pill.title=String(e); if(String(e)!==connectMidi.lastErr){ connectMidi.lastErr=String(e); log('MIDI: '+e); } } return; }
  if(!navigator.requestMIDIAccess)""")
R("pill.addEventListener('click',connectMidi);","""pill.addEventListener('click',connectMidi);
if(TAURI){ TAURI.event.listen('midi',e=>onMidi({data:new Uint8Array(e.payload.data)})); connectMidi(); setInterval(connectMidi,3000); }
// LED feedback for controllers with lights (Bàn DJ ver 3): same note numbers as the buttons
const LEDS=[[63,()=>D[1].playing],[69,()=>D[2].playing],[62,()=>!!D[1].track&&(!D[1].playing||D[1].cueHold)],[68,()=>!!D[2].track&&(!D[2].playing||D[2].cueHold)],
  [61,()=>D[1].sync],[65,()=>D[2].sync],[72,()=>FXU[1].slots[0].on],[71,()=>FXU[2].slots[0].on]], ledState={};
function syncLeds(){ if(!TAURI) return; for(const [n,f] of LEDS){ const v=f()?127:0; if(ledState[n]!==v){ ledState[n]=v; inv('midi_send',{data:[0x90,n,v]}).catch(()=>{}); } } }""")
R("  if(frame%6===0){ drawControls();","  if(frame%6===0){ drawControls(); syncLeds();")
R("open:{pl:true,rb:true}","open:{pl:true,rb:true,dir:true}")
# keep the original capitalisation of file names (FILES is keyed by lower-case name)
R("for(const f of FILES.keys()) trackForFile(f);","for(const [f,v] of FILES) trackForFile(v.name||f);")
R("for(const f of FILES.keys()) if(!byFile[f]) ids.push(trackForFile(f).id);","for(const [f,v] of FILES) if(!byFile[f]) ids.push(trackForFile(v.name||f).id);")
# F11 fullscreen
R("""document.addEventListener('keydown',e=>{
  if((e.ctrlKey||e.metaKey)&&e.key.toLowerCase()==='f')""","""document.addEventListener('keydown',e=>{
  if(e.key==='F11'&&TAURI){ e.preventDefault(); const w=TAURI.window.getCurrentWindow(); w.isFullscreen().then(f=>w.setFullscreen(!f)); return; }
  if((e.ctrlKey||e.metaKey)&&e.key.toLowerCase()==='f')""")
# start audio straight away in the app (the click gate stays as a fallback)
R("renderTree(); renderRows(); fit(); requestAnimationFrame(tick);","renderTree(); renderRows(); fit(); requestAnimationFrame(tick);\nif(TAURI){ ensureAudio(); setTimeout(audioStatus,400); }")
open(sys.argv[2],'w',encoding='utf-8').write(src)
print('ok',len(src))
