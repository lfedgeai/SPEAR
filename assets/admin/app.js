const qs=(sel)=>document.querySelector(sel)
const qsa=(sel)=>Array.from(document.querySelectorAll(sel))
let sse
async function fetchStats(){const r=await fetch('/admin/api/stats');const j=await r.json();qs('#stats').innerText=`Total: ${j.total_count}, Online: ${j.online_count}, Offline: ${j.offline_count}, Recent(60s): ${j.recent_60s_count}`}
async function fetchNodes(){const q=qs('#search').value.trim();const sort=qs('#sort').value;const [sort_by,order]=sort.split(':');const url=new URL('/admin/api/nodes',location.origin);if(q)url.searchParams.set('q',q);url.searchParams.set('sort_by',sort_by);url.searchParams.set('order',order);url.searchParams.set('limit','100');const r=await fetch(url);const j=await r.json();const tbody=qs('#nodes tbody');tbody.innerHTML='';(j.nodes||[]).forEach(n=>{const tr=document.createElement('tr');tr.innerHTML=`<td><a href="#" data-uuid="${n.uuid}">${n.uuid}</a></td><td>${n.ip_address}</td><td>${n.port}</td><td class="status-${n.status}">${n.status}</td><td>${n.last_heartbeat}</td>`;tbody.appendChild(tr)});qsa('#nodes a[data-uuid]').forEach(a=>a.addEventListener('click',e=>{e.preventDefault();showDetail(a.getAttribute('data-uuid'))}))}
async function showDetail(uuid){const r=await fetch(`/admin/api/nodes/${uuid}`);const j=await r.json();qs('#detail').textContent=JSON.stringify(j,null,2);qs('#modal').classList.remove('hidden')}
function closeModal(){qs('#modal').classList.add('hidden')}
function initEvents(){qs('#search').addEventListener('input',debounce(refresh,300));qs('#sort').addEventListener('change',refresh);qs('#close').addEventListener('click',closeModal)}
function debounce(fn,ms){let t;return(...args)=>{clearTimeout(t);t=setTimeout(()=>fn(...args),ms)}}
function initSSE(){try{sse=new EventSource('/admin/api/nodes/stream');sse.addEventListener('snapshot',()=>{refresh()})}catch(e){}}
async function refresh(){await fetchStats();await fetchNodes()}
async function start(){initEvents();initSSE();await refresh();setInterval(refresh,5000)}
start()
