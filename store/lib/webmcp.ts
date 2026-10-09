/**
 * WebMCP, as on the developer site (developer/lib/webmcp.ts): in a browser that offers `navigator.modelContext`, every
 * page registers two read-only tools an agent in the browser can call, backed by the same-origin Apps API, so a
 * signed-in visitor's agent sees what the visitor sees. Rendered inline by the root layout with the request's CSP
 * nonce; anywhere else it does nothing. The full set of tools is the MCP server at /mcp.
 */
export const WEBMCP_SCRIPT = `(function(){
if(!("modelContext" in navigator)||!navigator.modelContext)return;
function get(path){return fetch(path,{headers:{Accept:"application/json"},credentials:"same-origin"}).then(function(r){return r.json().catch(function(){return null}).then(function(body){if(!r.ok)throw new Error((body&&body.error&&body.error.message)||"The Apps API answered HTTP "+r.status+".");return body})})}
function result(value){return{content:[{type:"text",text:JSON.stringify(value)}],structuredContent:value}}
function summary(a){return{app_id:a.app_id,name:a.name,description:a.description,tags:a.tags,targets:a.targets,rating:a.rating,review_count:a.review_count,installs:a.installs,production_version:a.latest_production?a.latest_production.version:null,development_version:a.latest_development?a.latest_development.version:null,install:"silicon-apps install "+a.app_id,url:location.origin+"/apps/"+encodeURIComponent(a.app_id)}}
var tools=[{
name:"search_apps",
title:"Search Silicon Apps",
description:"Search the Silicon Apps store by app id, name, description or tag. Partial names and spelling mistakes still match, and exact ids and names come first. Returns each app with its install command.",
inputSchema:{type:"object",properties:{query:{type:"string",description:"What to look for, such as notes or briefcase. Leave it empty to list every app you can see."},limit:{type:"integer",minimum:1,maximum:50,description:"How many apps to return (10 by default)."}}},
annotations:{readOnlyHint:true},
execute:function(a){var q=String(a&&a.query||"");var n=Math.min(Math.max(parseInt(a&&a.limit,10)||10,1),50);return get("/v1/apps?limit="+n+(q?"&q="+encodeURIComponent(q):"")).then(function(d){return result({total:d.total,apps:d.items.map(summary)})})}
},{
name:"get_app",
title:"Get a Silicon Apps app",
description:"Get one app by its app_id: what it does, its authors, platforms, latest releases, who signed them, withdrawn releases, rating, installs, links and the command that installs it.",
inputSchema:{type:"object",properties:{app_id:{type:"string",description:"The app's permanent id, such as briefcase."}},required:["app_id"]},
annotations:{readOnlyHint:true},
execute:function(a){var id=String(a&&a.app_id||"").trim();if(!id)return Promise.reject(new Error("app_id is required: pass the app's id, such as briefcase."));return get("/v1/apps/"+encodeURIComponent(id)).then(function(app){var out=summary(app);out.authors=(app.authors||[]).map(function(x){return{id:x.id,display_name:x.display_name}});out.latest_production=app.latest_production;out.latest_development=app.latest_development;out.signed=!!app.signed;out.signed_by_author=!!app.signed_by_author;out.signed_by=app.signed_by||[];out.withdrawn_releases=app.withdrawn_releases||[];out.links=app.links||{};out.visibility=app.visibility;return result(out)})}
}];
try{if(typeof navigator.modelContext.registerTool==="function")tools.forEach(function(t){navigator.modelContext.registerTool(t)});else if(typeof navigator.modelContext.provideContext==="function")navigator.modelContext.provideContext({tools:tools})}catch(e){}
})();`;
