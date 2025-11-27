(() => {
  const { useState, useMemo, useEffect } = React;
  const { createRoot } = ReactDOM;
  const { Table, Layout, Input, Select, Tag, Modal, Statistic, Row, Col, Space, message, ConfigProvider, Card, Menu, Switch, Typography, Avatar } = antd;
  const { theme } = antd;
  const { QueryClient, QueryClientProvider, useQuery, useQueryClient } = ReactQuery;

  const queryClient = new QueryClient();
  const TZ_OPTIONS = ['system','UTC','Asia/Shanghai','Asia/Tokyo','Asia/Singapore','Asia/Kolkata','Asia/Hong_Kong','Europe/London','Europe/Berlin','Europe/Paris','Europe/Madrid','America/New_York','America/Los_Angeles','America/Chicago','America/Toronto','America/Sao_Paulo','Australia/Sydney'];

  function useAuthHeaders(){
    return useMemo(()=>{ const t=window.__ADMIN_TOKEN||''; return t?{Authorization:'Bearer '+t}:{ }; },[window.__ADMIN_TOKEN]);
  }

  async function fetchJSON(url, headers){ const r=await fetch(url,{headers}); if(!r.ok) throw new Error('HTTP '+r.status); return await r.json(); }

  function StatsBar(){
    const headers=useAuthHeaders();
    const { data, refetch }=useQuery(['stats'],()=>fetchJSON('/admin/api/stats',headers),{ refetchInterval:15000, refetchOnWindowFocus:false, refetchOnReconnect:false, staleTime:10000 });
    const lastRef=React.useRef(0);
    useEffect(()=>{ let es; if(!headers.Authorization){ try{ es=new EventSource('/admin/api/nodes/stream'); es.addEventListener('snapshot',()=>{ const now=Date.now(); if(now-lastRef.current>5000){ lastRef.current=now; refetch(); } }); }catch{} } return ()=>{ if(es) es.close(); }; },[headers.Authorization,refetch]);
    const stats=data||{ total_count:0, online_count:0, offline_count:0, recent_60s_count:0 };
    return React.createElement(Row,{gutter:12},
      React.createElement(Col,{xs:24, md:6}, React.createElement(Card,null, React.createElement(Statistic,{title:'Total', value:stats.total_count}))),
      React.createElement(Col,{xs:24, md:6}, React.createElement(Card,null, React.createElement(Statistic,{title:'Online', value:stats.online_count}))),
      React.createElement(Col,{xs:24, md:6}, React.createElement(Card,null, React.createElement(Statistic,{title:'Offline', value:stats.offline_count}))),
      React.createElement(Col,{xs:24, md:6}, React.createElement(Card,null, React.createElement(Statistic,{title:'Recent(60s)', value:stats.recent_60s_count})))
    );
  }

  function formatTs(ts,tz){ if(!ts||ts<=0) return ''; const d=dayjs(ts*1000); return tz&&tz!=='system'? d.tz(tz).format('YYYY-MM-DD HH:mm:ss') : d.format('YYYY-MM-DD HH:mm:ss'); }

  function NodesTable({tz}){
    const headers=useAuthHeaders(); const qc=useQueryClient();
    const [q,setQ]=useState(''); const [sort,setSort]=useState({field:'last_heartbeat',order:'desc'}); const [limit]=useState(100);
    const [detail,setDetail]=useState(null); const [tokenInput,setTokenInput]=useState(window.__ADMIN_TOKEN||''); const lastInvRef=React.useRef(0);
    const queryKey=['nodes',q,sort.field,sort.order,limit];
    const { data, refetch, isFetching }=useQuery(queryKey, async()=>{ const url=new URL('/admin/api/nodes',location.origin); if(q) url.searchParams.set('q',q); url.searchParams.set('sort_by',sort.field); url.searchParams.set('order',sort.order); url.searchParams.set('limit',String(limit)); return await fetchJSON(url.toString(),headers); },{ keepPreviousData:true, refetchOnWindowFocus:false, refetchOnReconnect:false, staleTime:10000 });
    useEffect(()=>{ let es; if(!headers.Authorization){ try{ es=new EventSource('/admin/api/nodes/stream'); es.addEventListener('snapshot',()=>{ const now=Date.now(); if(now-lastInvRef.current>5000){ lastInvRef.current=now; qc.invalidateQueries({queryKey}); } }); }catch{} } return ()=>{ if(es) es.close(); }; },[headers.Authorization,qc,queryKey]);
    async function openDetail(uuid){ try{ const j=await fetchJSON('/admin/api/nodes/'+uuid,headers); setDetail(j); } catch(e){ message.error('Load detail failed'); } }
    const columns=[
      { title:'UUID', dataIndex:'uuid', key:'uuid', render:(v)=>React.createElement('a',{onClick:()=>openDetail(v)},v) },
      { title:'Name', dataIndex:'name', key:'name' },
      { title:'IP', dataIndex:'ip_address', key:'ip_address' },
      { title:'Port', dataIndex:'port', key:'port' },
      { title:'Status', dataIndex:'status', key:'status', render:(s)=>React.createElement(Tag,{color:(s==='online'||s==='active')?'green':'red'},s) },
      { title:'Last Heartbeat', dataIndex:'last_heartbeat', key:'last_heartbeat', render:(v)=>formatTs(v,tz) },
      { title:'Registered At', dataIndex:'registered_at', key:'registered_at', render:(v)=>formatTs(v,tz) },
    ];
    const rows=(data&&data.nodes)||[];
    const toolbar=React.createElement(Space,{style:{marginBottom:12}},
      React.createElement(Input,{placeholder:'Search', allowClear:true, value:q, onChange:(e)=>setQ(e.target.value), style:{width:240}}),
      React.createElement(Select,{value:`${sort.field}:${sort.order}`, style:{width:220}, onChange:(v)=>{ const [f,o]=v.split(':'); setSort({field:f,order:o}); }},
        React.createElement(Select.Option,{value:'last_heartbeat:desc'},'Last Heartbeat ↓'),
        React.createElement(Select.Option,{value:'last_heartbeat:asc'},'Last Heartbeat ↑'),
        React.createElement(Select.Option,{value:'registered_at:desc'},'Registered At ↓'),
        React.createElement(Select.Option,{value:'registered_at:asc'},'Registered At ↑')
      ),
      React.createElement(Input,{placeholder:'Admin Token (optional)', value:tokenInput, onChange:(e)=>setTokenInput(e.target.value), style:{width:260}}),
      React.createElement('button',{onClick:()=>{ window.__ADMIN_TOKEN=tokenInput; localStorage.setItem('ADMIN_TOKEN',tokenInput); refetch(); }},'Apply Token')
    );
    return React.createElement(React.Fragment,null,
      toolbar,
      React.createElement(Table,{rowKey:'uuid', columns, dataSource:rows, loading:isFetching, pagination:{pageSize:50}}),
      React.createElement(Modal,{open:!!detail,onCancel:()=>setDetail(null),footer:null,width:720},React.createElement('pre',null,detail?JSON.stringify(detail,null,2):''))
    );
  }

  function TaskCreateModal({open,onClose,onCreated}){
    const headers=useAuthHeaders();
    const [form,setForm]=useState({ name:'', description:'', priority:'normal', node_uuid:'', endpoint:'', version:'', executable_type:'', executable_name:'', executable_uri:'', checksum:'', capabilities:'', args:'', env:'' });
    const { data: nodesData }=ReactQuery.useQuery(['admin-nodes-for-create'], async()=>{ const url=new URL('/admin/api/nodes',location.origin); url.searchParams.set('limit','200'); url.searchParams.set('order','desc'); url.searchParams.set('sort_by','last_heartbeat'); return await fetchJSON(url.toString(), headers); }, { refetchOnWindowFocus:false, staleTime:15000 });
    useEffect(()=>{ if(!form.node_uuid && nodesData && nodesData.nodes && nodesData.nodes.length>0){ const online = nodesData.nodes.find(n=> (n.status||'').toLowerCase()==='online'); const first = online || nodesData.nodes[0]; setForm(f=>({...f, node_uuid:first.uuid})); } },[nodesData]);
    async function submit(){
      const caps=form.capabilities? form.capabilities.split(',').map(s=>s.trim()).filter(Boolean):[];
      const args=form.args? form.args.split(',').map(s=>s.trim()).filter(Boolean):[];
      const envLines=form.env? form.env.split('\n').map(s=>s.trim()).filter(Boolean):[];
      const envObj={}; envLines.forEach(l=>{ const i=l.indexOf('='); if(i>0){ envObj[l.slice(0,i).trim()]=l.slice(i+1).trim(); }});
      const payload={
        name: form.name,
        description: form.description,
        priority: form.priority,
        node_uuid: form.node_uuid,
        endpoint: form.endpoint,
        version: form.version,
        capabilities: caps,
        executable: form.executable_type? { type: form.executable_type, name: form.executable_name||undefined, uri: form.executable_uri||undefined, checksum_sha256: form.checksum||undefined, args, env: envObj }: undefined,
      };
      try{
        const r=await fetch('/admin/api/tasks',{ method:'POST', headers:{ 'content-type':'application/json', ...headers }, body: JSON.stringify(payload)});
        const j=await r.json();
        if(!j.success) throw new Error(j.message||'Failed');
        message.success('Task created: '+j.task_id);
        onCreated&&onCreated(); onClose();
      }catch(e){ message.error('Create failed: '+e.message); }
    }
    return React.createElement(Modal,{open,onCancel:onClose,onOk:submit,title:'Create Task'},
      React.createElement(Space,{direction:'vertical',style:{width:'100%'}},
        React.createElement(Input,{placeholder:'Name',value:form.name,onChange:e=>setForm({...form,name:e.target.value})}),
        React.createElement(Input,{placeholder:'Description',value:form.description,onChange:e=>setForm({...form,description:e.target.value})}),
        React.createElement(Select,{value:form.priority,onChange:v=>setForm({...form,priority:v}),style:{width:'100%'}},
          React.createElement(Select.Option,{value:'low'},'Low'),
          React.createElement(Select.Option,{value:'normal'},'Normal'),
          React.createElement(Select.Option,{value:'high'},'High'),
          React.createElement(Select.Option,{value:'urgent'},'Urgent')
        ),
        React.createElement(Select,{value:form.node_uuid,onChange:v=>setForm({...form,node_uuid:v}),style:{width:'100%'}},
          (nodesData&&nodesData.nodes||[]).map(n=>React.createElement(Select.Option,{key:n.uuid,value:n.uuid}, (n.name? (n.name+' '):'')+n.uuid ))
        ),
        React.createElement(Input,{placeholder:'Endpoint',value:form.endpoint,onChange:e=>setForm({...form,endpoint:e.target.value})}),
        React.createElement(Input,{placeholder:'Version',value:form.version,onChange:e=>setForm({...form,version:e.target.value})}),
        React.createElement(Input,{placeholder:'Capabilities (comma separated)',value:form.capabilities,onChange:e=>setForm({...form,capabilities:e.target.value})}),
        React.createElement(Select,{value:form.executable_type,onChange:v=>setForm({...form,executable_type:v}),style:{width:'100%'}},
          React.createElement(Select.Option,{value:''},'No Executable'),
          React.createElement(Select.Option,{value:'binary'},'Binary'),
          React.createElement(Select.Option,{value:'script'},'Script'),
          React.createElement(Select.Option,{value:'container'},'Container'),
          React.createElement(Select.Option,{value:'wasm'},'WASM'),
          React.createElement(Select.Option,{value:'process'},'Process')
        ),
        React.createElement(Input,{placeholder:'Executable Name (optional)',value:form.executable_name,onChange:e=>setForm({...form,executable_name:e.target.value})}),
        React.createElement(Input,{placeholder:'Executable URI (optional)',value:form.executable_uri,onChange:e=>setForm({...form,executable_uri:e.target.value})}),
        React.createElement(Input,{placeholder:'Checksum SHA256 (optional)',value:form.checksum,onChange:e=>setForm({...form,checksum:e.target.value})}),
        React.createElement(Input.TextArea,{placeholder:'Args (comma separated)',value:form.args,onChange:e=>setForm({...form,args:e.target.value})}),
        React.createElement(Input.TextArea,{placeholder:'Env (key=value per line)',value:form.env,onChange:e=>setForm({...form,env:e.target.value})})
      )
    );
  }

  function TasksTable({tz}){
    const headers=useAuthHeaders();
    const [q,setQ]=useState('');
    const [sort,setSort]=useState({field:'registered_at',order:'desc'});
    const [limit]=useState(100);
    const [creating,setCreating]=useState(false);
    const [detail,setDetail]=useState(null);
    const queryKey=['tasks',q,sort.field,sort.order,limit];
    const { data, refetch, isFetching }=useQuery(queryKey, async()=>{ const url=new URL('/admin/api/tasks',location.origin); if(q) url.searchParams.set('q',q); url.searchParams.set('sort_by',sort.field); url.searchParams.set('order',sort.order); url.searchParams.set('limit',String(limit)); return await fetchJSON(url.toString(),headers); },{ keepPreviousData:true, refetchOnWindowFocus:false, refetchOnReconnect:false, staleTime:10000 });
    const statusTag=(s)=>React.createElement(Tag,{color:(s==='active'||s==='registered')?'blue':(s==='inactive'?'orange':'default')},s);
    async function openDetail(taskId){ try{ const j=await fetchJSON('/admin/api/tasks/'+taskId, headers); setDetail(j); } catch(e){ message.error('Load detail failed'); } }
    const columns=[
      { title:'Task ID', dataIndex:'task_id', key:'task_id', render:(v)=>React.createElement('a',{onClick:()=>openDetail(v)},v) },
      { title:'Name', dataIndex:'name', key:'name' },
      { title:'Exec Type', dataIndex:'executable_type', key:'executable_type' },
      { title:'Exec Name', dataIndex:'executable_name', key:'executable_name' },
      { title:'Exec URI', dataIndex:'executable_uri', key:'executable_uri' },
      { title:'Node UUID', dataIndex:'node_uuid', key:'node_uuid' },
      { title:'Status', dataIndex:'status', key:'status', render:statusTag },
      { title:'Priority', dataIndex:'priority', key:'priority' },
      { title:'Endpoint', dataIndex:'endpoint', key:'endpoint' },
      { title:'Version', dataIndex:'version', key:'version' },
      { title:'Registered At', dataIndex:'registered_at', key:'registered_at', render:(v)=>formatTs(v,tz) },
      { title:'Last Heartbeat', dataIndex:'last_heartbeat', key:'last_heartbeat', render:(v)=>formatTs(v,tz) },
    ];
    const rows=(data&&data.tasks)||[];
    const toolbar=React.createElement(Space,{style:{marginBottom:12}},
      React.createElement(Input,{placeholder:'Search', allowClear:true, value:q, onChange:(e)=>setQ(e.target.value), style:{width:240}}),
      React.createElement(Select,{value:`${sort.field}:${sort.order}`, style:{width:220}, onChange:(v)=>{ const [f,o]=v.split(':'); setSort({field:f,order:o}); }},
        React.createElement(Select.Option,{value:'registered_at:desc'},'Registered At ↓'),
        React.createElement(Select.Option,{value:'registered_at:asc'},'Registered At ↑'),
        React.createElement(Select.Option,{value:'last_heartbeat:desc'},'Last Heartbeat ↓'),
        React.createElement(Select.Option,{value:'last_heartbeat:asc'},'Last Heartbeat ↑')
      ),
      React.createElement('button',{onClick:()=>setCreating(true)},'Create Task')
    );
    return React.createElement(React.Fragment,null,
      toolbar,
      React.createElement(Table,{rowKey:'task_id', columns, dataSource:rows, loading:isFetching, pagination:{pageSize:50}}),
      React.createElement(Modal,{open:!!detail,onCancel:()=>setDetail(null),footer:null,width:720},React.createElement('pre',null,detail?JSON.stringify(detail,null,2):'')),
      React.createElement(TaskCreateModal,{open:creating,onClose:()=>setCreating(false),onCreated:()=>{refetch();}})
    );
  }

  function SettingsPage({themeMode,setThemeMode,tz,setTz}){
    const [tokenInput,setTokenInput]=useState(localStorage.getItem('ADMIN_TOKEN')||'');
    return React.createElement('div',{className:'page'},
      React.createElement(Card,{title:'Appearance'}, React.createElement(Space,{align:'center'}, React.createElement(Typography.Text,null,'Dark Mode'), React.createElement(Switch,{checked:themeMode==='dark',onChange:(v)=>{ const m=v?'dark':'light'; setThemeMode(m); localStorage.setItem('ADMIN_THEME',m); } }))),
      React.createElement(Card,{title:'Timezone',style:{marginTop:16}}, React.createElement(Space,null, React.createElement(Select,{value:tz,style:{width:280},onChange:(v)=>{ setTz(v); localStorage.setItem('ADMIN_TZ',v); } }, TZ_OPTIONS.map((z)=>React.createElement(Select.Option,{key:z,value:z}, z==='system'?'System Default':z))))),
      React.createElement(Card,{title:'Admin Token',style:{marginTop:16}}, React.createElement(Space,null, React.createElement(Input,{placeholder:'Admin Token',value:tokenInput,onChange:(e)=>setTokenInput(e.target.value),style:{width:320}}), React.createElement('button',{onClick:()=>{ window.__ADMIN_TOKEN=tokenInput; localStorage.setItem('ADMIN_TOKEN',tokenInput); message.success('Token applied'); }},'Apply')))
    );
  }

  function App(){
    const [route,setRoute]=useState(window.location.hash.slice(1)||'nodes');
    const [themeMode,setThemeMode]=useState(localStorage.getItem('ADMIN_THEME')||'light');
    const [tz,setTz]=useState(localStorage.getItem('ADMIN_TZ')||'system');
    useEffect(()=>{ const onHash=()=>setRoute(window.location.hash.slice(1)||'nodes'); window.addEventListener('hashchange',onHash); return()=>window.removeEventListener('hashchange',onHash); },[]);
    const algo=themeMode==='dark'? theme.darkAlgorithm : theme.defaultAlgorithm;
    const tzLabel=tz==='system'? dayjs.tz.guess() : tz;
    function TopBar(){ const { token }=antd.theme.useToken(); return React.createElement('div',{className:'topbar',style:{height:56, background:token.colorBgContainer}}, React.createElement('div',{className:'topbar-left'}, React.createElement(Typography.Text,{style:{color:token.colorText,fontWeight:600}},'SMS Admin')), React.createElement('div',{className:'topbar-right'}, React.createElement(Tag,null,'TZ: '+tzLabel), React.createElement(Avatar,{style:{backgroundColor:token.colorPrimary}},'UA'), React.createElement(Typography.Text,{type:'secondary'},'Profile')) ); }
    return React.createElement(ConfigProvider,{theme:{algorithm:algo}}, React.createElement(Layout,{style:{minHeight:'100%'}}, React.createElement(Layout.Sider,{theme:themeMode==='dark'?'dark':'light', style:{borderInlineEnd:'none'}}, React.createElement('div',{style:{color:themeMode==='dark'?'#fff':undefined,padding:16,fontWeight:600}},'SMS Admin'), React.createElement(Menu,{theme:themeMode==='dark'?'dark':'light',selectedKeys:[route],onClick:(e)=>{window.location.hash=e.key;}}, React.createElement(Menu.Item,{key:'nodes'},'Nodes'), React.createElement(Menu.Item,{key:'tasks'},'Tasks'), React.createElement(Menu.Item,{key:'settings'},'Settings') )), React.createElement(Layout,null, React.createElement(Layout.Header,{style:{padding:0, margin:0, height:56, lineHeight:'56px', background:'transparent', borderBottom:'0', boxShadow:'none'}}, React.createElement(TopBar)), React.createElement(Layout.Content,{style:{background:'transparent'}}, route==='nodes'? React.createElement('div',{className:'page'}, React.createElement(StatsBar), React.createElement(NodesTable,{tz})) : route==='tasks'? React.createElement('div',{className:'page'}, React.createElement(TasksTable,{tz})) : React.createElement(SettingsPage,{themeMode,setThemeMode,tz,setTz}) ))));
  }

  const root=createRoot(document.getElementById('root'));
  root.render(React.createElement(QueryClientProvider,{client:queryClient}, React.createElement(App)));
})();
