'use strict';
// Repository text is data. Never interpolate it into HTML, styles, URLs, or code.
const $ = id => document.getElementById(id);
const node = (tag, className, text) => { const el=document.createElement(tag); if(className)el.className=className; if(text!==undefined)el.textContent=String(text); return el; };
const clear = el => el.replaceChildren();
const array = value => Array.isArray(value)?value:[];
const short = value => value ? String(value).slice(0,12) : 'Unavailable';
const state = { status:null, selected:null, searchSequence:0, evidenceSequence:0, view:'overview' };
const views = {
  overview:['Inspect the source','Knowledge, with evidence.','Explore the repository snapshot behind your agent’s context.'],
  search:['Find the source','Every result has a source.','Search the frozen repository snapshot, then inspect its exact source fragment.'],
  dependencies:['Follow the relationships','See what connects.','Trace the static relationships the compiler observed, with unresolved references shown.'],
  changes:['Compare the evidence','A record of what changed.','Compare the files and source objects in two recorded project snapshots.']
};
function error(message){$('error').textContent=String(message);$('error').hidden=false;}
function clearError(){$('error').hidden=true;$('error').textContent='';}
async function api(operation,arguments_={}) {
  const response=await fetch('api/query',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({operation,arguments:arguments_}),credentials:'omit',cache:'no-store',redirect:'error'});
  let value;try{value=await response.json();}catch{throw new Error('The local session is unavailable. Keep the viewer command running and reopen its session URL.');}
  if(!response.ok)throw new Error(value.error||`Query failed (${response.status}).`);
  return value;
}
function showView(name) {
  if(!views[name])name='overview';state.view=name;
  for(const el of document.querySelectorAll('.view'))el.hidden=el.id!==name;
  for(const el of document.querySelectorAll('[data-view]')){const active=el.dataset.view===name;el.classList.toggle('active',active);if(active)el.setAttribute('aria-current','page');else el.removeAttribute('aria-current');}
  const [kicker,title,description]=views[name];$('view-kicker').textContent=kicker;$('view-title').textContent=title;$('view-description').textContent=description;
  history.replaceState(null,'',`#${name}`);
}
function empty(container,title,description){clear(container);const el=node('div','empty');el.append(node('h2','',title),node('p','',description));container.append(el);}
function details(container,entries) {clear(container);for(const [label,value]of entries){const row=node('div');row.append(node('dt','',label),node('dd','',value??'Not recorded'));container.append(row);}}
function metric(label,value,note) {const el=node('div','metric');el.append(node('span','eyebrow',label),node('strong','',Number.isFinite(value)?value.toLocaleString():'—'),node('small','',note));return el;}
function coverage(status) {
  const el=$('coverage');clear(el);const languages=status.coverage?.languages;
  if(languages){const row=node('div','diagnostic');row.append(node('span','tag','Sources'));const label=Array.isArray(languages)?languages.join(', '):Object.entries(languages).map(([name,count])=>`${name}: ${count}`).join(' · ');row.append(node('span','',label||'No supported source objects'));el.append(row);}
  const unresolved=Number(status.coverage?.unresolved||0);const row=node('div','diagnostic');row.append(node('span','tag','Static analysis'),node('span','',`${unresolved.toLocaleString()} unresolved relationship${unresolved===1?'':'s'}. Dynamic calls, macros and runtime behavior may fall outside this analysis.`));el.append(row);
  const diagnostics=array(status.diagnostics);
  if(!diagnostics.length){const row=node('div','diagnostic');row.append(node('span','tag','Compiler'),node('span','', 'No diagnostics recorded for this snapshot. Supported source coverage is not a guarantee of complete program understanding.'));el.append(row);}
  for(const item of diagnostics.slice(0,100)){const row=node('div','diagnostic');row.append(node('span','tag',item.code||'Notice'),node('span','',`${item.path?`${item.path} — `:''}${item.message||'No detail recorded'}`));el.append(row);}
  if(diagnostics.length>100)el.append(node('p','muted',`Showing 100 of ${diagnostics.length} diagnostics. Use knowledge status --json for the full record.`));
}
function snapshotOptions(status) {
  const rows=array(status.snapshots);for(const id of ['older','newer']){const selected=$(id).value;clear($(id));for(const item of rows){const option=node('option','',`${short(item.id)} · ${item.dirty?'working tree':short(item.revision)} · ${item.files??'?'} files`);option.value=item.id;$(id).append(option);}if(rows.some(item=>item.id===selected))$(id).value=selected;}
  if(rows.length){$('newer').value=rows[0].id;$('older').value=rows.length>1?rows[1].id:rows[0].id;}
  $('changes-form').querySelector('button').disabled=rows.length<2;
  if(rows.length<2)empty($('changes-results'),'One snapshot recorded','Compile this repository again after an edit. Both immutable snapshots will be available here.');
}
async function loadStatus(){
  clearError();$('refresh').disabled=true;
  try{
    const status=await api('status');state.status=status;
    const project=status.project||{};const root=project.root||'';const name=root.split(/[\\/]/).filter(Boolean).pop()||project.id||'Compiled project';
    $('project-name').textContent=name;$('project-id').textContent=short(project.id);
    const selected=array(status.snapshots).find(item=>item.id===status.snapshot_id)||{};const c=status.coverage||{};
    clear($('metrics'));$('metrics').append(metric('Source files',c.files,'Frozen in this snapshot'),metric('Source objects',c.objects,'Symbols and sections'),metric('Relationships',c.relationships,`${c.resolved??0} resolved references`),metric('Snapshots',array(status.snapshots).length,'Recorded in this project'));
    const freshness=status.freshness||{state:'unavailable',reason:'Source freshness was not checked.'};
    const freshnessLabel={current:'Source current',stale:'Source changed',unavailable:'Freshness unavailable'}[freshness.state]||'Freshness unavailable';
    $('snapshot-state').textContent=freshnessLabel;
    const changes=freshness.changes;const changeSummary=changes?`${changes.added} added · ${changes.modified} modified · ${changes.removed} removed${freshness.config_changed?' · capture policy changed':''}${freshness.ignore_rules_changed?' · ignore rules changed':''}`:'';
    details($('snapshot-details'),[['Snapshot',status.snapshot_id],['Revision',selected.revision||'No Git revision'],['Source',root],['Capture',selected.dirty?'Includes uncommitted source':'Recorded source snapshot'],['Source freshness',freshness.reason],...(changeSummary?[['Changes',changeSummary]]:[])]);
    $('snapshot-footer').textContent=`Snapshot ${short(status.snapshot_id)} · Local evidence`;
    $('verify').disabled=!status.snapshot_id;$('verify-result').textContent='';coverage(status);snapshotOptions(status);
  }catch(e){error(e.message);$('snapshot-state').textContent='Unavailable';$('project-name').textContent='Project unavailable';}
  finally{$('refresh').disabled=false;}
}
function resultCard(item){
  const object=item.object||item;const evidence=item.evidence||{};const button=node('button','result');button.type='button';button.dataset.object=object.id||'';
  const head=node('div','result-head');head.append(node('h3','',object.name||object.path||'Source object'),node('span','tag',object.kind||'source'));button.append(head);
  button.append(node('span','result-path',`${object.path||evidence.path||''}:${object.start_line||evidence.start_line||1}${object.end_line?`–${object.end_line}`:''}`));
  if(object.signature||evidence.fragment)button.append(node('p','',(object.signature||evidence.fragment).slice(0,280)));
  button.addEventListener('click',()=>openObject(object.id));return button;
}
async function search(event){event?.preventDefault();const text=$('query').value.trim();if(!text)return;
  const sequence=++state.searchSequence;clearError();$('search-meta').textContent='Searching the local snapshot…';$('search-form').querySelector('button').disabled=true;
  try{const data=await api('search',{query:text,limit:50});if(sequence!==state.searchSequence)return;const rows=array(data.results);clear($('results'));$('search-meta').textContent=`${rows.length} shown${data.total!==undefined?` of ${data.total}`:''} · snapshot ${short(data.snapshot_id)}`;if(!rows.length)empty($('results'),'No matching source','Try a symbol name, file path or a shorter phrase. Unsupported or excluded files are not searched.');for(const item of rows)$('results').append(resultCard(item));}
  catch(e){if(sequence===state.searchSequence){error(e.message);$('search-meta').textContent='Search could not complete.';}}
  finally{if(sequence===state.searchSequence)$('search-form').querySelector('button').disabled=false;}
}
function metadata(container,entries){const el=node('div','metadata');for(const[label,value]of entries){const row=node('div');row.append(node('b','',`${label} `),document.createTextNode(value==null?'Not recorded':String(value)));el.append(row);}container.append(el);}
async function openObject(id){if(!id)return;const sequence=++state.evidenceSequence;state.selected=id;showView('search');clearError();empty($('evidence'),'Loading source','Reading the stored evidence for this object.');
  for(const el of document.querySelectorAll('[data-object]'))el.classList.toggle('selected',el.dataset.object===id);
  try{const data=await api('get',{id});if(sequence!==state.evidenceSequence)return;const object=data.object||{};const evidence=data.evidence||{};const el=$('evidence');clear(el);const head=node('div','panel-heading');head.append(node('h2','',object.name||'Source evidence'),node('span','tag',object.language||object.kind||'source'));el.append(head);
    metadata(el,[['File',evidence.path||object.path],['Lines',`${evidence.start_line??object.start_line}–${evidence.end_line??object.end_line}`],['Snapshot',evidence.snapshot_id||data.snapshot_id],['Revision',evidence.revision||'Working source'],['Object',object.id],['Source SHA-256',evidence.source_sha256||object.source_sha256]]);
    const pre=node('pre');const code=node('code','',evidence.fragment??'No source fragment returned.');pre.append(code);el.append(pre);
    const button=node('button','button secondary','Trace this object');button.type='button';button.addEventListener('click',()=>{$('object-id').value=id;showView('dependencies');dependencies();});el.append(button);
    for(const text of array(evidence.limitations))el.append(node('p','muted',text));
    if(window.innerWidth<1100)el.scrollIntoView({behavior:'smooth',block:'start'});
  }catch(e){if(sequence===state.evidenceSequence){error(e.message);empty($('evidence'),'Source unavailable','This object may belong to another snapshot. Refresh and search again.');}}
}
function dependencyNode(id,label,unresolved=false){const el=node(id?'button':'span',`node${unresolved?' unresolved':''}`,label);if(id){el.type='button';el.addEventListener('click',()=>openObject(id));}return el;}
async function dependencies(event){event?.preventDefault();const id=$('object-id').value.trim();if(!id)return;clearError();const button=$('dependency-form').querySelector('button');button.disabled=true;
  try{const data=await api('dependencies',{id,depth:2,direction:$('direction').value});const el=$('dependency-results');clear(el);const objects=new Map(array(data.objects).map(o=>[o.id,o]));if(data.root)objects.set(data.root.id,data.root);const relations=array(data.relationships);const unresolved=array(data.unresolved);const all=[...relations];for(const item of unresolved){if(!all.some(r=>r.from===item.from&&r.to===item.to&&r.kind===item.kind&&r.target===item.target&&r.path===item.path&&r.line===item.line))all.push(item);}
    const head=node('div','panel-heading');head.append(node('h2','',data.root?.name||'Observed relationships'),node('span','tag',`${all.length} relationships`));el.append(head);
    if(!all.length){const note=node('div','empty');note.append(node('h2','','No relationships recorded'),node('p','','The compiler found no matching relationships within this trace. Dynamic behavior may remain outside its coverage.'));el.append(note);}
    for(const item of all.slice(0,200)){const row=node('div','relation');const source=objects.get(item.from);const target=objects.get(item.to);row.append(dependencyNode(source?.id,source?.name||item.from||'Source'),node('span','relation-kind',`${item.kind||'reference'} →`),dependencyNode(target?.id,target?.name||item.target||'Unresolved',!item.to));el.append(row);const provenance=node('p','result-path',`${item.path||''}:${item.line||''} · ${item.resolution||(!item.to?'unresolved':'resolved')}`);el.append(provenance);}
    if(all.length>200)el.append(node('p','muted',`Showing 200 of ${all.length} relationships. Narrow the trace with the CLI.`));for(const text of array(data.limitations))el.append(node('p','muted',text));
  }catch(e){error(e.message);}finally{button.disabled=false;}
}
async function changes(event){event?.preventDefault();const from=$('older').value;const to=$('newer').value;if(!from||!to)return;clearError();if(from===to){error('Choose two different snapshots to compare.');return;}const button=$('changes-form').querySelector('button');button.disabled=true;
  try{const data=await api('changes',{from,to});const el=$('changes-results');clear(el);const head=node('div','panel-heading');head.append(node('h2','','Snapshot comparison'),node('span','tag',`${short(data.from)} → ${short(data.to)}`));el.append(head);
    let count=0;for(const [key,label]of [['added','Added files'],['modified','Modified files'],['removed','Removed files'],['objects_added','Added objects'],['objects_removed','Removed objects']]){const items=array(data[key]);count+=items.length;const group=node('section','change-group');const title=node('h3','',label);title.append(node('span','tag',items.length));group.append(title);if(!items.length)group.append(node('p','muted','None'));for(const item of items.slice(0,100)){const text=typeof item==='string'?item:`${item.name||item.id} · ${item.path||''}:${item.start_line||''}`;group.append(node('div','change-row',text));}if(items.length>100)group.append(node('p','muted',`Showing 100 of ${items.length}. Read the full diff with the CLI.`));el.append(group);}if(!count)el.prepend(node('p','muted','No source changes between these snapshots.'));
  }catch(e){error(e.message);}finally{button.disabled=false;}
}
$('search-form').addEventListener('submit',search);$('dependency-form').addEventListener('submit',dependencies);$('changes-form').addEventListener('submit',changes);
$('refresh').addEventListener('click',loadStatus);
$('verify').addEventListener('click',async()=>{clearError();$('verify').disabled=true;$('verify-result').textContent='Verifying snapshot…';try{const result=await api('verify',{snapshot:state.status.snapshot_id});$('verify-result').textContent=result.valid?'Snapshot integrity verified.':'Verification did not pass.';}catch(e){$('verify-result').textContent='Verification failed.';error(e.message);}finally{$('verify').disabled=false;}});
for(const el of document.querySelectorAll('[data-view]'))el.addEventListener('click',()=>showView(el.dataset.view));
window.addEventListener('hashchange',()=>showView(location.hash.slice(1)));
showView(location.hash.slice(1)||'overview');loadStatus();
