//! Embedded War-Room console. Single file, zero build step (Law I/III).

pub const HTML: &str = r#"<!doctype html>
<html><head><meta charset="utf-8"><title>Bellona War Room</title>
<style>
 body{background:#0b0e14;color:#c9d1d9;font-family:ui-monospace,monospace;margin:0;padding:1rem}
 h1{color:#ff6a3d;font-size:1.2rem} h2{font-size:.85rem;color:#8b949e;text-transform:uppercase;letter-spacing:.08em}
 .cols{display:flex;gap:1rem;flex-wrap:wrap} .col{flex:1;min-width:320px;background:#11151f;border:1px solid #21262d;border-radius:8px;padding:.75rem}
 #feed{height:45vh;overflow:auto;font-size:.75rem;line-height:1.5}
 button{background:#238636;color:#fff;border:0;padding:.4rem .7rem;border-radius:6px;cursor:pointer}
 button.deny{background:#b62324}
 input,textarea{width:100%;background:#0b0e14;color:#c9d1d9;border:1px solid #30363d;border-radius:6px;padding:.35rem}
 .row{display:flex;gap:.4rem;margin:.3rem 0}
 .ok{color:#3fb950}.bad{color:#f85149}.ticket{border-top:1px dashed #30363d;padding:.4rem 0}
</style></head><body>
<h1>⚔ BELLONA — WAR ROOM</h1>
<div class="cols">
 <div class="col"><h2>Campaign</h2>
  <textarea id="goal" rows="3" placeholder="state the goal…"></textarea>
  <div class="row"><button onclick="run()">Launch campaign</button></div>
 </div>
 <div class="col"><h2>Pending approvals (take the wheel)</h2><div id="tickets"></div>
  <div class="row"><button onclick="veto()" class="deny">TRIBUNICIAN VETO</button></div>
 </div>
 <div class="col"><h2>Live events</h2><div id="feed"></div></div>
</div>
<script>
async function refreshTickets(){
  const r = await fetch('/v1/gate/pending'); const j = await r.json();
  document.getElementById('tickets').innerHTML = (j.tickets||[]).map(t=>
    `<div class="ticket">${t.ticket_id}<br>${t.tool} — ${t.intent}<br>
     <button onclick="decide('${t.ticket_id}',true)">approve</button>
     <button class="deny" onclick="decide('${t.ticket_id}',false)">reject</button></div>`).join('') || '<i>nothing parked</i>';
}
async function decide(id, ok){
  await fetch(ok?'/v1/gate/approve':'/v1/gate/reject',{method:'POST',
    headers:{'content-type':'application/json'},
    body:JSON.stringify({ticket_id:id, approver:'war-room', reason: ok?'':'human override'})});
  refreshTickets();
}
async function veto(){ await fetch('/v1/veto',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({reason:'war-room veto'})}); }
async function run(){
  const goal = document.getElementById('goal').value.trim(); if(!goal) return;
  await fetch('/v1/runs',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({goal})});
}
const es = new EventSource('/v1/events');
es.onmessage = e => { const d = document.getElementById('feed');
  d.innerHTML += `<div>${new Date().toLocaleTimeString()} ${e.data.replace(/</g,'&lt;')}</div>`;
  d.scrollTop = d.scrollHeight; };
refreshTickets(); setInterval(refreshTickets, 2000);
</script></body></html>"#;

pub fn html() -> &'static str {
    HTML
}
