use crate::app::AppState;
use axum::{
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use std::sync::Arc;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(index))
        .route("/admin", get(admin))
        .route("/health", get(health))
}

async fn index() -> impl IntoResponse {
    Html(
        r#"<!doctype html>
<html lang="zh-CN">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Picbad</title>
<style>
:root{--canvas:#faf9f5;--ink:#141413;--body:#3d3d3a;--muted:#6c6a64;--primary:#cc785c;--primary-active:#a9583e;--card:#efe9de;--hairline:#e6dfd8;--dark:#181715;--on-dark:#faf9f5}
*{box-sizing:border-box}body{margin:0;background:var(--canvas);color:var(--ink);font-family:Inter,-apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,sans-serif}
.mark{width:20px;height:20px;position:relative;display:inline-block}.mark:before,.mark:after{content:"";position:absolute;left:9px;top:1px;width:2px;height:18px;background:var(--ink);border-radius:2px}.mark:after{transform:rotate(90deg)}.mark i:before,.mark i:after{content:"";position:absolute;left:9px;top:1px;width:2px;height:18px;background:var(--ink);border-radius:2px}.mark i:before{transform:rotate(45deg)}.mark i:after{transform:rotate(-45deg)}
nav{height:64px;display:flex;align-items:center;justify-content:space-between;max-width:1200px;margin:0 auto;padding:0 24px}
.brand{display:flex;align-items:center;gap:10px;font-weight:600}.navlinks{display:flex;gap:22px;color:var(--muted);font-size:14px}.btn{height:40px;display:inline-flex;align-items:center;justify-content:center;border-radius:8px;border:1px solid var(--hairline);padding:0 18px;text-decoration:none;color:var(--ink);font-size:14px;font-weight:500}.btn.primary{background:var(--primary);border-color:var(--primary);color:white}
main{max-width:1200px;margin:0 auto;padding:82px 24px 96px;display:grid;grid-template-columns:minmax(0,1fr) 460px;gap:56px;align-items:center}
h1{font-family:"Cormorant Garamond","EB Garamond",Garamond,"Times New Roman",serif;font-size:68px;line-height:1.02;letter-spacing:-1.5px;font-weight:500;margin:0 0 22px}
p{font-size:18px;line-height:1.65;color:var(--body);margin:0 0 28px;max-width:620px}.actions{display:flex;gap:12px;flex-wrap:wrap}
.mock{background:var(--dark);color:var(--on-dark);border-radius:16px;padding:24px;min-height:390px}.mock-top{display:flex;gap:8px;margin-bottom:22px}.dot{width:10px;height:10px;border-radius:50%;background:#cc785c}.dot:nth-child(2){background:#e8a55a}.dot:nth-child(3){background:#5db8a6}
.code{font-family:"JetBrains Mono",Consolas,monospace;font-size:13px;line-height:1.75;color:#a09d96;background:#1f1e1b;border-radius:12px;padding:18px;overflow:auto}.code b{color:#faf9f5;font-weight:400}.code em{color:#cc785c;font-style:normal}
.strip{max-width:1200px;margin:0 auto 72px;padding:0 24px;display:grid;grid-template-columns:repeat(3,1fr);gap:16px}.tile{background:var(--card);border-radius:12px;padding:24px}.tile h2{font-size:18px;margin:0 0 8px}.tile p{font-size:15px;margin:0;color:var(--muted)}
@media(max-width:860px){.navlinks{display:none}main{grid-template-columns:1fr;padding-top:42px}h1{font-size:42px}.strip{grid-template-columns:1fr}.mock{min-height:auto}}
</style>
</head>
<body>
<nav><div class="brand"><span class="mark"><i></i></span><span>Picbad</span></div><div class="navlinks"><span>Upload</span><span>Transform</span><span>Cache</span><span>Users</span></div><a class="btn primary" href="/admin">打开面板</a></nav>
<main>
  <section>
    <h1>一个安静、快速、可管理的 Rust 图床。</h1>
    <p>Picbad 将上传去重、实时转换、智能缓存和多用户密钥管理收进同一个温暖的工作界面。它更像一张编辑台，而不是一块冷冰冰的控制板。</p>
    <div class="actions"><a class="btn primary" href="/admin">进入管理面板</a><a class="btn" href="/api/status">查看状态 API</a></div>
  </section>
  <aside class="mock">
    <div class="mock-top"><span class="dot"></span><span class="dot"></span><span class="dot"></span></div>
    <div class="code"><b>POST</b> /api/images<br><em>x-api-key:</em> pk_user_upload_key<br><br>{<br>&nbsp;&nbsp;"duplicate": false,<br>&nbsp;&nbsp;"url": "/i/image_id",<br>&nbsp;&nbsp;"sha256": "..."<br>}<br><br><b>GET</b> /i/image_id?w=1200&fmt=webp&q=85</div>
  </aside>
</main>
<section class="strip">
  <article class="tile"><h2>密钥上传</h2><p>每个用户拥有独立上传密钥，管理 Token 与上传密钥分离。</p></article>
  <article class="tile"><h2>实时转换</h2><p>通过 URL 参数转换尺寸、格式和 JPEG 质量。</p></article>
  <article class="tile"><h2>缓存节制</h2><p>转换结果进入 LRU 缓存，缓存可在管理面板清理。</p></article>
</section>
</body>
</html>"#,
    )
}

async fn health() -> impl IntoResponse {
    "ok"
}

async fn admin() -> impl IntoResponse {
    Html(ADMIN_HTML)
}

const ADMIN_HTML: &str = r#"<!doctype html>
<html lang="zh-CN">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Picbad 管理面板</title>
<style>
:root{color-scheme:light;--canvas:#faf9f5;--soft:#f5f0e8;--card:#efe9de;--cream-strong:#e8e0d2;--hairline:#e6dfd8;--hairline-soft:#ebe6df;--ink:#141413;--body:#3d3d3a;--muted:#6c6a64;--muted-soft:#8e8b82;--primary:#cc785c;--primary-active:#a9583e;--amber:#e8a55a;--teal:#5db8a6;--dark:#181715;--dark-elevated:#252320;--dark-soft:#1f1e1b;--on-dark:#faf9f5;--on-dark-soft:#a09d96;--error:#c64545;--success:#5db872}
*{box-sizing:border-box}body{margin:0;background:var(--canvas);color:var(--ink);font-family:Inter,-apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,sans-serif}button,input,select{font:inherit}
.hidden{display:none!important}.display{font-family:"Cormorant Garamond","EB Garamond",Garamond,"Times New Roman",serif;font-weight:500;letter-spacing:-.03em}.mono{font-family:"JetBrains Mono",Consolas,ui-monospace,monospace}
.mark{width:20px;height:20px;position:relative;display:inline-block;flex:0 0 auto}.mark:before,.mark:after{content:"";position:absolute;left:9px;top:1px;width:2px;height:18px;background:currentColor;border-radius:2px}.mark:after{transform:rotate(90deg)}.mark i:before,.mark i:after{content:"";position:absolute;left:9px;top:1px;width:2px;height:18px;background:currentColor;border-radius:2px}.mark i:before{transform:rotate(45deg)}.mark i:after{transform:rotate(-45deg)}
button{min-height:40px;border-radius:8px;border:1px solid var(--hairline);background:var(--canvas);color:var(--ink);padding:0 14px;font-size:14px;font-weight:500;cursor:pointer}button.primary{background:var(--primary);border-color:var(--primary);color:white}button.primary:active{background:var(--primary-active)}button.dark{background:var(--dark-elevated);border-color:#34312d;color:var(--on-dark)}button.danger{border-color:#e5beb6;color:var(--error);background:#fff8f5}button.link{border:0;background:transparent;padding:0;min-height:0;color:var(--primary)}
input,select{width:100%;height:40px;border:1px solid var(--hairline);border-radius:8px;background:var(--canvas);color:var(--ink);padding:0 12px;font-size:14px}input:focus,select:focus{outline:3px solid rgba(204,120,92,.16);border-color:var(--primary)}label{display:block;margin:14px 0 7px;color:var(--muted);font-size:12px;font-weight:500}
.login-wrap{min-height:100vh;display:grid;place-items:center;padding:24px}.login-card{width:min(920px,100%);display:grid;grid-template-columns:1fr 380px;gap:0;background:var(--dark);border-radius:16px;overflow:hidden}.login-copy{padding:48px;color:var(--on-dark)}.login-copy h1{font-size:48px;line-height:1.08;margin:26px 0 14px}.login-copy p{color:var(--on-dark-soft);line-height:1.65;margin:0}.login-form{background:var(--canvas);padding:42px}.login-form h2{font-size:28px;line-height:1.15;margin:0 0 20px}
.shell{min-height:100vh}.topnav{height:64px;display:flex;align-items:center;justify-content:space-between;border-bottom:1px solid var(--hairline);padding:0 24px}.brand{display:flex;align-items:center;gap:10px;font-weight:600}.nav-actions{display:flex;gap:10px;align-items:center}
.hero{max-width:1200px;margin:0 auto;padding:48px 24px 28px;display:grid;grid-template-columns:minmax(0,1fr) 390px;gap:30px;align-items:stretch}.hero h1{font-size:48px;line-height:1.08;margin:0 0 12px}.hero p{margin:0;color:var(--body);font-size:16px;line-height:1.6;max-width:660px}.key-panel{background:var(--dark);color:var(--on-dark);border-radius:16px;padding:24px}.key-panel .eyebrow{color:var(--on-dark-soft);font-size:12px;margin-bottom:10px}.key-panel code{display:block;color:var(--on-dark);background:var(--dark-soft);border-radius:12px;padding:14px;word-break:break-all;font-size:12px;line-height:1.55;margin-bottom:14px}.key-actions{display:flex;gap:8px;flex-wrap:wrap}
.layout{max-width:1200px;margin:0 auto;padding:0 24px 80px;display:grid;gap:20px}.stats{display:grid;grid-template-columns:repeat(4,minmax(0,1fr));gap:16px}.metric{background:var(--card);border-radius:12px;padding:24px}.metric b{display:block;font-size:34px;line-height:1;margin-bottom:10px}.metric span{font-size:13px;color:var(--muted)}
.workgrid{display:grid;grid-template-columns:minmax(300px,340px) minmax(0,1fr);gap:20px;align-items:start}.stack{display:grid;gap:20px;min-width:0}.panel{background:var(--canvas);border:1px solid var(--hairline);border-radius:12px;padding:24px;min-width:0}.panel.soft{background:var(--card);border:0}.panel.dark{background:var(--dark);border:0;color:var(--on-dark)}.panel h2{font-size:22px;line-height:1.2;margin:0 0 16px}.panel p.note,.notice{font-size:13px;line-height:1.55;color:var(--muted);margin:10px 0 0;word-break:break-word}.panel.dark .notice{color:var(--on-dark-soft)}
.upload-drop{border:1px dashed #d7c8b9;border-radius:12px;background:rgba(250,249,245,.55);padding:18px;display:grid;gap:12px;transition:border-color .15s ease,background .15s ease;min-width:0}.upload-drop.dragging{border-color:var(--primary);background:#fff3ed}.file-input{position:absolute;inline-size:1px;block-size:1px;opacity:0;pointer-events:none}.file-picker{display:grid;grid-template-columns:minmax(0,1fr) auto;align-items:center;gap:10px;width:100%;max-width:100%;min-width:0;overflow:hidden;border:1px solid var(--hairline);border-radius:12px;background:var(--canvas);padding:10px}.file-copy{min-width:0;overflow:hidden}.file-copy strong{display:block;font-size:14px;margin-bottom:3px;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}.file-copy span{display:block;color:var(--muted);font-size:12px;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}.file-button{width:auto;min-width:76px;padding:0 10px;white-space:nowrap}.drop-hint{font-size:13px;color:var(--muted);line-height:1.5;margin:0}
.toolbar{display:flex;align-items:flex-start;justify-content:space-between;gap:14px;margin-bottom:12px;flex-wrap:wrap}.status-pill{display:inline-flex;align-items:center;gap:8px;background:var(--cream-strong);border-radius:9999px;padding:6px 12px;color:var(--body);font-size:13px;max-width:100%}.status-dot{width:8px;height:8px;border-radius:50%;background:var(--success);flex:0 0 auto}
.table-wrap{overflow:auto;max-width:100%;border-radius:12px;border:1px solid var(--hairline);background:var(--canvas)}table{width:100%;border-collapse:collapse;font-size:13px;min-width:700px}th,td{text-align:left;border-bottom:1px solid var(--hairline-soft);padding:12px;vertical-align:middle}th{color:var(--muted);font-weight:500;background:var(--soft)}tr:last-child td{border-bottom:0}.thumb{width:48px;height:48px;object-fit:cover;border-radius:8px;background:var(--card);border:1px solid var(--hairline)}.video-thumb{background:#141413}a{color:var(--primary);text-decoration:none}.table-wrap .mono{display:inline-block;max-width:260px;overflow:hidden;text-overflow:ellipsis;vertical-align:bottom}.error{color:var(--error)!important}.ok{color:var(--primary);font-weight:500}
.code-window{background:var(--dark-soft);border-radius:12px;padding:16px;color:var(--on-dark-soft);font-size:12px;line-height:1.7;overflow:auto}.code-window b{color:var(--on-dark);font-weight:400}.code-window em{color:var(--primary);font-style:normal}.users{margin-top:0}
@media(max-width:1100px){.workgrid{grid-template-columns:1fr}.table-wrap .mono{max-width:420px}}
@media(max-width:900px){.login-card,.hero{grid-template-columns:1fr}.stats{grid-template-columns:1fr 1fr}.hero{padding-top:28px}.hero h1{font-size:36px}.login-copy,.login-form{padding:28px}.topnav{align-items:flex-start;height:auto;padding:16px;gap:12px;flex-direction:column}.nav-actions{width:100%;flex-wrap:wrap}}
@media(max-width:560px){.stats{grid-template-columns:1fr}.key-actions,.nav-actions{display:grid;grid-template-columns:1fr;width:100%}button{width:100%}.layout,.hero{padding-left:16px;padding-right:16px}}
</style>
</head>
<body>
<div id="loginView" class="login-wrap">
  <div class="login-card">
    <section class="login-copy">
      <span class="mark"><i></i></span>
      <h1 class="display">Picbad 管理台</h1>
      <p>用密钥上传图片，用参数实时转换格式与尺寸，在一个安静的界面里管理缓存、用户和存储。</p>
    </section>
    <section class="login-form">
      <h2 class="display">登录</h2>
      <label>用户名</label><input id="username" autocomplete="username">
      <label>密码</label><input id="password" type="password" autocomplete="current-password">
      <p><button class="primary" id="login">登录</button> <button id="register" type="button">注册</button></p>
      <div class="notice error" id="loginMsg"></div>
    </section>
  </div>
</div>
<div id="appView" class="shell hidden">
  <nav class="topnav">
    <div class="brand"><span class="mark"><i></i></span><span>Picbad</span></div>
    <div class="nav-actions"><button id="refresh">刷新</button><button class="danger" id="clearCache">清空缓存</button><button id="logout">退出</button></div>
  </nav>
  <header class="hero">
    <section>
      <h1 class="display">图片、密钥与缓存，放在一张温暖的工作台上。</h1>
      <p id="who">-</p>
    </section>
    <aside class="key-panel">
      <div class="eyebrow">当前用户上传密钥</div>
      <code id="myKey">-</code>
      <div class="key-actions"><button class="dark" id="copyKey">复制密钥</button><button class="dark" id="rotateMyKey">轮换密钥</button></div>
    </aside>
  </header>
  <main class="layout">
    <section class="stats">
      <div class="metric"><b class="display" id="statImages">-</b><span>图片数量</span></div>
      <div class="metric"><b class="display" id="statUsers">-</b><span>用户数量</span></div>
      <div class="metric"><b class="display" id="statStored">-</b><span>原图存储</span></div>
      <div class="metric"><b class="display" id="statCache">-</b><span>转换缓存</span></div>
    </section>
    <section class="workgrid">
      <div class="stack">
        <section class="panel soft">
          <h2 class="display">上传图片</h2>
          <div class="upload-drop" id="dropZone">
            <input class="file-input" id="file" type="file" accept="image/jpeg,image/png,image/gif,image/webp,image/avif,image/x-icon,video/mp4">
            <div class="file-picker">
              <div class="file-copy">
                <strong id="fileTitle">选择图片文件</strong>
                <span id="fileName">支持 JPEG、PNG、GIF、WebP、AVIF、ICO、MP4，默认上限 50MB</span>
              </div>
              <button class="file-button" id="chooseFile" type="button">选择文件</button>
            </div>
            <p class="drop-hint">也可以直接从桌面拖入图片，松手后会自动上传。</p>
          </div>
          <p><button class="primary" id="upload">使用密钥上传</button></p>
          <div class="notice" id="uploadMsg"></div>
        </section>
        <section class="panel dark">
          <h2 class="display">API 速记</h2>
          <div class="code-window mono"><b>POST</b> /api/images<br><em>x-api-key:</em> pk_...<br><br><b>DELETE</b> /api/images/:id<br><em>authorization:</em> Bearer token<br><br><b>GET</b> /api/status</div>
        </section>
        <section id="userPanel" class="panel hidden">
          <h2 class="display">创建用户</h2>
          <label>用户名</label><input id="newUsername">
          <label>密码</label><input id="newPassword" type="password">
          <label>角色</label><select id="newRole"><option value="user">user</option><option value="admin">admin</option></select>
          <p><button id="createUser">创建用户</button></p>
          <div class="notice" id="userMsg"></div>
        </section>
      </div>
      <section class="panel">
        <div class="toolbar">
          <h2 class="display" id="galleryTitle">我的图库</h2>
          <span class="status-pill"><i class="status-dot"></i><span id="statusLine">读取状态中</span></span>
        </div>
        <div class="table-wrap"><table><thead><tr><th>预览</th><th>文件</th><th>归属</th><th>尺寸</th><th>大小</th><th>访问</th><th>操作</th></tr></thead><tbody id="images"></tbody></table></div>
      </section>
    </section>
    <section id="usersBox" class="panel users hidden">
      <h2 class="display">用户与密钥</h2>
      <div class="table-wrap"><table><thead><tr><th>用户名</th><th>角色</th><th>上传密钥</th><th>最近登录</th><th>操作</th></tr></thead><tbody id="users"></tbody></table></div>
    </section>
  </main>
</div>
<script>
const $=id=>document.getElementById(id);
let token=localStorage.getItem('picbad_token')||'';let me=null;
const fmt=n=>n>1024*1024?`${(n/1024/1024).toFixed(1)} MB`:n>1024?`${(n/1024).toFixed(1)} KB`:`${n} B`;
const esc=s=>String(s??'').replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
let selectedFile=null;
const preview=x=>x.ext==='mp4'?`<video class="thumb video-thumb" src="${x.url}" muted preload="metadata" playsinline></video>`:`<img class="thumb" src="${x.url}?w=96&h=96&fmt=webp">`;
const mediaLink=x=>x.ext==='mp4'?(x.player_url||`/p/${x.id}`):x.url;
const mediaSize=x=>x.ext==='mp4'?'视频':`${x.width||'-'} x ${x.height||'-'}`;
async function api(path,opts={}){const headers=opts.headers||{};if(token)headers.authorization=`Bearer ${token}`;const res=await fetch(path,{...opts,headers});if(!res.ok)throw new Error((await res.json().catch(()=>({error:res.statusText}))).error);return res.headers.get('content-type')?.includes('json')?res.json():res.text()}
function showApp(on){$('loginView').classList.toggle('hidden',on);$('appView').classList.toggle('hidden',!on)}
async function load(){
  if(!token){showApp(false);return}
  try{
    me=await api('/api/me');showApp(true);
    $('myKey').textContent=me.api_key;$('who').textContent=`${me.username} / ${me.role}`;
    $('userPanel').classList.toggle('hidden',me.role!=='admin');$('usersBox').classList.toggle('hidden',me.role!=='admin');
    $('galleryTitle').textContent=me.role==='admin'?'全部图库':'我的图库';
    const [stats,imgs,status]=await Promise.all([api('/api/stats'),api('/api/images'),api('/api/status')]);
    $('statImages').textContent=stats.images;$('statUsers').textContent=stats.users;$('statStored').textContent=fmt(stats.stored_bytes);$('statCache').textContent=fmt(stats.cache.bytes);
    $('statusLine').textContent=`状态正常 / ${status.supported_formats.join(', ')}`;
    $('images').innerHTML=imgs.map(x=>`<tr><td>${preview(x)}</td><td>${esc(x.file_name)}<br><a class="mono" href="${mediaLink(x)}" target="_blank">${mediaLink(x)}</a></td><td>${esc(x.owner_username||me.username)}</td><td>${mediaSize(x)}</td><td>${fmt(x.size_bytes)}</td><td>${x.hits}</td><td><button data-del="${x.id}" class="danger">删除</button></td></tr>`).join('');
    document.querySelectorAll('[data-del]').forEach(b=>b.onclick=()=>deleteImage(b.dataset.del));
    if(me.role==='admin'){const users=await api('/api/users');$('users').innerHTML=users.map(u=>`<tr><td>${esc(u.username)}</td><td>${u.role}</td><td class="mono">${u.api_key}</td><td>${u.last_login_at||'-'}</td><td><button data-rotate="${u.id}">轮换</button> ${u.role==='user'&&u.id!==me.id?`<button class="danger" data-del-user="${u.id}">删除</button>`:''}</td></tr>`).join('');document.querySelectorAll('[data-rotate]').forEach(b=>b.onclick=()=>rotateKey(b.dataset.rotate));document.querySelectorAll('[data-del-user]').forEach(b=>b.onclick=()=>deleteUser(b.dataset.delUser))}
  }catch(e){localStorage.removeItem('picbad_token');token='';showApp(false);$('loginMsg').textContent=e.message}
}
async function rotateKey(id){const r=await api(`/api/users/${id}/key`,{method:'POST'});if(id===me.id)me.api_key=r.api_key;await load()}
async function deleteImage(id){if(!confirm('确定删除这张图片？'))return;await api(`/api/images/${id}`,{method:'DELETE'});await load()}
async function deleteUser(id){if(!confirm('确定删除这个用户及其全部资源？'))return;await api(`/api/users/${id}`,{method:'DELETE'});await load()}
function setSelectedFile(file){selectedFile=file;$('fileName').textContent=file?`${file.name} / ${fmt(file.size)}`:'支持 JPEG、PNG、GIF、WebP、AVIF、ICO、MP4，默认上限 50MB';$('fileTitle').textContent=file?'已选择文件':'选择图片或视频文件'}
async function uploadFile(file){if(!file)return;if(!me?.api_key){$('uploadMsg').textContent='请先登录';$('uploadMsg').className='notice error';return}setSelectedFile(file);const fd=new FormData();fd.append('file',file);try{$('uploadMsg').textContent='正在上传...';$('uploadMsg').className='notice';const res=await fetch('/api/images',{method:'POST',headers:{'x-api-key':me.api_key},body:fd});if(!res.ok)throw new Error((await res.json()).error);const r=await res.json();$('uploadMsg').innerHTML=`<span class="ok">已上传</span> <a href="${r.url}" target="_blank">${r.url}</a>`;await load()}catch(e){$('uploadMsg').textContent=e.message;$('uploadMsg').className='notice error'}}
$('login').onclick=async()=>{try{const r=await api('/api/login',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({username:$('username').value,password:$('password').value})});token=r.token;localStorage.setItem('picbad_token',token);$('loginMsg').textContent='';await load()}catch(e){$('loginMsg').textContent=e.message}};
$('register').onclick=async()=>{try{const r=await api('/api/register',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({username:$('username').value,password:$('password').value})});token=r.token;localStorage.setItem('picbad_token',token);$('loginMsg').textContent='';await load()}catch(e){$('loginMsg').textContent=e.message}};
$('logout').onclick=()=>{localStorage.removeItem('picbad_token');token='';me=null;showApp(false)};
$('refresh').onclick=load;$('clearCache').onclick=async()=>{await api('/api/cache',{method:'DELETE'});await load()};
$('copyKey').onclick=async()=>{await navigator.clipboard.writeText(me.api_key);$('statusLine').textContent='密钥已复制'};
$('rotateMyKey').onclick=()=>rotateKey(me.id);
$('chooseFile').onclick=()=>$('file').click();
$('file').onchange=()=>setSelectedFile($('file').files[0]||null);
$('upload').onclick=()=>uploadFile(selectedFile||$('file').files[0]);
['dragenter','dragover'].forEach(ev=>$('dropZone').addEventListener(ev,e=>{e.preventDefault();e.stopPropagation();$('dropZone').classList.add('dragging')}));
['dragleave','drop'].forEach(ev=>$('dropZone').addEventListener(ev,e=>{e.preventDefault();e.stopPropagation();$('dropZone').classList.remove('dragging')}));
$('dropZone').addEventListener('drop',e=>{const file=[...(e.dataTransfer?.files||[])].find(f=>f.type.startsWith('image/')||f.type==='video/mp4'||/\.(jpe?g|png|gif|webp|avif|ico|mp4)$/i.test(f.name));if(file)uploadFile(file);else{$('uploadMsg').textContent='请拖入支持的图片或 MP4 文件';$('uploadMsg').className='notice error'}});
$('createUser').onclick=async()=>{try{const r=await api('/api/users',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({username:$('newUsername').value,password:$('newPassword').value,role:$('newRole').value})});$('userMsg').textContent=`已创建，上传密钥：${r.api_key}`;await load()}catch(e){$('userMsg').textContent=e.message;$('userMsg').className='notice error'}};
load();
</script>
</body>
</html>"#;
