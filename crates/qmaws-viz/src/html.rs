//! `interactive_tree.html` (plan 4.8.4): one self-contained file with
//! inline data and JavaScript and no external resources. Zoom (mouse
//! wheel, buttons), pan (drag), taxon search, tooltips (support, halo
//! value, group) and a switch between the circular and the rectangular
//! layout; branch colours follow S1, S2 or nothing.

use crate::colour::{hex, support_colour};
use crate::tree::{escape, halo_colour, node_support, EdgeSupport, Groups, TreeLayout};
use std::collections::BTreeMap;
use std::fmt::Write as _;

/// A JSON string literal, safe inside a `<script>` element.
fn js(s: &str) -> String {
    let mut out = String::from("\"");
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '<' => out.push_str("\\u003c"),
            '>' => out.push_str("\\u003e"),
            '&' => out.push_str("\\u0026"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn num(v: Option<f64>) -> String {
    match v.filter(|x| x.is_finite()) {
        Some(x) => format!("{x:.4}"),
        None => "null".into(),
    }
}

/// The page. `supports` are the support sets offered (for example S1 and
/// S2); the first is shown at the start.
pub fn interactive_html(
    layout: &TreeLayout,
    halo: &BTreeMap<String, Option<f64>>,
    supports: &[EdgeSupport],
    groups: Option<&Groups>,
    title: &[String],
) -> String {
    let all = layout.taxa();
    let mut data = String::from("{\"nodes\":[");
    for (v, node) in layout.nodes.iter().enumerate() {
        if v > 0 {
            data.push(',');
        }
        let name = node.name.as_deref().filter(|_| node.children.is_empty());
        let _ = write!(
            data,
            "{{\"p\":{},\"c\":[{}],\"n\":{},\"a\":{:.6},\"d\":{},\"k\":{}",
            node.parent.map_or("null".into(), |p| p.to_string()),
            node.children
                .iter()
                .map(|c| c.to_string())
                .collect::<Vec<_>>()
                .join(","),
            name.map_or("null".into(), js),
            node.angle,
            node.depth,
            layout.clade(v).len()
        );
        if let Some(n) = name {
            let _ = write!(data, ",\"h\":{}", num(halo.get(n).copied().flatten()));
            if let Some(g) = groups.and_then(|gr| gr.of.get(n)) {
                let _ = write!(data, ",\"g\":{}", js(g));
            }
        }
        let _ = write!(data, ",\"s\":[");
        for (i, sup) in supports.iter().enumerate() {
            if i > 0 {
                data.push(',');
            }
            data.push_str(&num(node_support(layout, v, &all, sup)));
        }
        data.push_str("]}");
    }
    let _ = write!(data, "],\"maxDepth\":{},\"supports\":[", layout.max_depth);
    data.push_str(
        &supports
            .iter()
            .map(|s| js(&s.label))
            .collect::<Vec<_>>()
            .join(","),
    );
    data.push_str("],\"groupColours\":{");
    if let Some(gr) = groups {
        data.push_str(
            &gr.names()
                .iter()
                .map(|g| format!("{}:{}", js(g), js(&hex(gr.colour_of(g)))))
                .collect::<Vec<_>>()
                .join(","),
        );
    }
    let _ = write!(
        data,
        "}},\"groupSource\":{},\"haloScale\":[{}],\"supportScale\":[{}]}}",
        js(groups.map_or("", |g| g.source.as_str())),
        (0..=20)
            .map(|i| js(&hex(halo_colour(Some(i as f64 / 20.0)))))
            .collect::<Vec<_>>()
            .join(","),
        (0..=20)
            .map(|i| js(&hex(support_colour(Some(i as f64 / 20.0)))))
            .collect::<Vec<_>>()
            .join(",")
    );
    let heading = title
        .first()
        .cloned()
        .unwrap_or_else(|| "Q-MAWS tree".into());
    let subtitle = title[1.min(title.len())..].join(" \u{00b7} ");
    let mut page = String::new();
    let _ = write!(
        page,
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{}</title>
<style>{STYLE}</style>
</head>
<body>
<header>
<h1>{}</h1>
<p class="sub">{}</p>
<div class="controls">
<button id="layout" type="button">Rectangular layout</button>
<label>Branch colour <select id="support"></select></label>
<label>Find taxon <input id="search" type="search" placeholder="name"></label>
<span id="hits"></span>
<button id="zin" type="button">Zoom in</button><button id="zout" type="button">Zoom out</button><button id="reset" type="button">Reset view</button>
</div>
</header>
<main><svg id="view" xmlns="http://www.w3.org/2000/svg" role="img" aria-label="Tree"></svg><div id="tip" role="status"></div></main>
<footer id="legend"></footer>
<script id="tree-data" type="application/json">{data}</script>
<script>{SCRIPT}</script>
</body>
</html>
"#,
        escape(&heading),
        escape(&heading),
        escape(&subtitle),
    );
    page
}

const STYLE: &str = r###"
body{margin:0;font-family:system-ui,-apple-system,"Segoe UI",sans-serif;background:#fff;color:#222}
header{padding:8px 14px;border-bottom:1px solid #ddd}
h1{font-size:18px;margin:2px 0}
.sub{margin:2px 0 6px;color:#555;font-size:13px}
.controls{display:flex;flex-wrap:wrap;gap:10px;align-items:center;font-size:13px}
button,select,input{font:inherit;font-size:13px}
main{position:relative}
#view{display:block;width:100%;height:calc(100vh - 150px);min-height:400px;cursor:grab;background:#fff}
#view.drag{cursor:grabbing}
#tip{position:absolute;pointer-events:none;background:#222;color:#fff;padding:5px 8px;border-radius:4px;font-size:12px;display:none;white-space:pre}
footer{padding:6px 14px;font-size:12px;color:#444;border-top:1px solid #ddd}
.leaf text{font-size:11px}
.hit .nm{font-weight:bold;fill:#d55e00}
.hit .mark{stroke:#d55e00;stroke-width:2}
.sw{display:inline-block;width:12px;height:10px;margin:0 4px 0 10px;border:1px solid #666;vertical-align:middle}
.bar{display:inline-block;width:110px;height:10px;border:1px solid #666;vertical-align:middle;margin:0 4px}
"###;

const SCRIPT: &str = r###"
(function(){
"use strict";
var D=JSON.parse(document.getElementById("tree-data").textContent);
var NS="http://www.w3.org/2000/svg";
var svg=document.getElementById("view"),tip=document.getElementById("tip");
var circular=true,supportIndex=D.supports.length?0:-1,query="";
var view=null,base=null;
function el(name,attrs,parent){var e=document.createElementNS(NS,name);for(var k in attrs){e.setAttribute(k,attrs[k]);}if(parent){parent.appendChild(e);}return e;}
function scale(list,v){if(v===null||v===undefined){return "#969696";}v=Math.max(0,Math.min(1,v));return list[Math.round(v*(list.length-1))];}
function supportColour(v){if(v===null||v===undefined){return "#969696";}return scale(D.supportScale,v);}
function supportWidth(v){return 1+3*(v===null||v===undefined?0:Math.max(0,Math.min(1,v)));}
var leaves=[];(function walk(v){var n=D.nodes[v];if(!n.c.length){leaves.push(v);}else{for(var i=0;i<n.c.length;i++){walk(n.c[i]);}}})(0);
var longest=0;leaves.forEach(function(l){longest=Math.max(longest,D.nodes[l].n.length);});
function fmt(v){return v===null||v===undefined?"no value":v.toFixed(3);}
function info(v){var n=D.nodes[v];var lines=[];
 if(!n.c.length){lines.push(n.n);lines.push("Halo value: "+fmt(n.h));if(n.g){lines.push("Group: "+n.g);}}
 else{lines.push("Internal branch above "+n.k+" taxa");for(var i=0;i<D.supports.length;i++){lines.push(D.supports[i]+": "+fmt(n.s[i]));}}
 return lines.join("\n");}
function hover(e,v){tip.textContent=info(v);tip.style.display="block";var r=svg.getBoundingClientRect();tip.style.left=(e.clientX-r.left+14)+"px";tip.style.top=(e.clientY-r.top+14)+"px";}
function unhover(){tip.style.display="none";}
function bind(e,v){e.addEventListener("mousemove",function(ev){hover(ev,v);});e.addEventListener("mouseleave",unhover);}
function draw(){
 while(svg.firstChild){svg.removeChild(svg.firstChild);}
 var g=el("g",{},svg);var nL=leaves.length;
 var pos={};
 if(circular){
  var R=Math.max(260,nL*5.5),lab=longest*6.2+20,size=2*(R+lab+60);
  var c=size/2;
  base=[0,0,size,size];
  D.nodes.forEach(function(n,v){var r=n.c.length?R*n.d/Math.max(1,D.maxDepth):R;pos[v]={a:n.a,r:r};});
  function P(a,r){return [c+r*Math.cos(a),c+r*Math.sin(a)];}
  D.nodes.forEach(function(n,v){n.c.forEach(function(ch){
   var col="#555",w=1.2,sv=supportIndex>=0?D.nodes[ch].s[supportIndex]:null;
   if(D.nodes[ch].c.length&&supportIndex>=0){col=supportColour(sv);w=supportWidth(sv);}
   var r=pos[v].r,a0=n.a,a1=D.nodes[ch].a,p0=P(a0,r),p1=P(a1,r),p2=P(a1,pos[ch].r);
   var d="M"+p0[0]+","+p0[1];
   if(r>0&&Math.abs(a1-a0)>1e-9){d+=" A"+r+","+r+" 0 "+(Math.abs(a1-a0)>Math.PI?1:0)+" "+(a1>a0?1:0)+" "+p1[0]+","+p1[1];}else{d+=" L"+p1[0]+","+p1[1];}
   d+=" L"+p2[0]+","+p2[1];
   var e=el("path",{d:d,fill:"none",stroke:col,"stroke-width":w,"stroke-linecap":"round"},g);
   var hit=el("path",{d:d,fill:"none",stroke:"transparent","stroke-width":8},g);bind(hit,ch);});});
  var half=Math.PI/nL;
  leaves.forEach(function(l){var n=D.nodes[l],a=n.a,deg=a*180/Math.PI,left=Math.cos(a)<0;
   var grp=el("g",{"class":"leaf","data-name":n.n.toLowerCase()},g);
   var p=P(a,R+6);var t=el("text",{"class":"nm",x:p[0],y:p[1],"text-anchor":left?"end":"start","dominant-baseline":"central",transform:"rotate("+(left?deg+180:deg)+" "+p[0]+" "+p[1]+")"},grp);t.textContent=n.n;
   var ro=R+lab+16,ri=ro-12;var q0=P(a-half*0.88,ro),q1=P(a+half*0.88,ro),q2=P(a+half*0.88,ri),q3=P(a-half*0.88,ri);
   el("path",{"class":"mark",d:"M"+q0+" A"+ro+","+ro+" 0 0 1 "+q1+" L"+q2+" A"+ri+","+ri+" 0 0 0 "+q3+" Z",fill:scale(D.haloScale,n.h),stroke:"#666","stroke-width":0.5},grp);
   if(n.g){var go=ro+16,gi=go-8;var r0=P(a-half,go),r1=P(a+half,go),r2=P(a+half,gi),r3=P(a-half,gi);el("path",{d:"M"+r0+" A"+go+","+go+" 0 0 1 "+r1+" L"+r2+" A"+gi+","+gi+" 0 0 0 "+r3+" Z",fill:D.groupColours[n.g]||"#999"},grp);}
   bind(grp,l);});
 }else{
  var row=16,W=Math.max(600,D.maxDepth*40),labw=longest*6.6+40,H=nL*row+40;
  base=[0,0,W+labw+200,H];
  var y={},x={};leaves.forEach(function(l,i){y[l]=20+row*(i+0.5);});
  (function post(v){var n=D.nodes[v];if(n.c.length){var s=0;n.c.forEach(function(ch){post(ch);s+=y[ch];});y[v]=s/n.c.length;}x[v]=20+W*(n.c.length?n.d/Math.max(1,D.maxDepth):1);})(0);
  D.nodes.forEach(function(n,v){n.c.forEach(function(ch){
   var col="#444",w=1.2,sv=supportIndex>=0?D.nodes[ch].s[supportIndex]:null;
   if(D.nodes[ch].c.length&&supportIndex>=0){col=supportColour(sv);w=supportWidth(sv);}
   var d="M"+x[v]+","+y[v]+" V"+y[ch]+" H"+x[ch];
   el("path",{d:d,fill:"none",stroke:col,"stroke-width":w},g);
   var hit=el("path",{d:d,fill:"none",stroke:"transparent","stroke-width":8},g);bind(hit,ch);});});
  leaves.forEach(function(l){var n=D.nodes[l];var grp=el("g",{"class":"leaf","data-name":n.n.toLowerCase()},g);
   el("rect",{"class":"mark",x:x[l]+4,y:y[l]-5,width:10,height:10,fill:scale(D.haloScale,n.h),stroke:"#666","stroke-width":0.5},grp);
   var t=el("text",{"class":"nm",x:x[l]+18,y:y[l],"dominant-baseline":"central"},grp);t.textContent=n.n;
   if(n.g){el("rect",{x:x[l]+labw,y:y[l]-row/2,width:12,height:row,fill:D.groupColours[n.g]||"#999"},grp);var gt=el("text",{x:x[l]+labw+16,y:y[l],"dominant-baseline":"central","font-style":"italic","font-size":10},grp);gt.textContent=n.g;}
   bind(grp,l);});
 }
 view=base.slice();setView();applySearch();
}
function setView(){svg.setAttribute("viewBox",view.join(" "));}
function zoom(f,cx,cy){if(cx===undefined){cx=view[0]+view[2]/2;cy=view[1]+view[3]/2;}view=[cx-(cx-view[0])*f,cy-(cy-view[1])*f,view[2]*f,view[3]*f];setView();}
svg.addEventListener("wheel",function(e){e.preventDefault();var r=svg.getBoundingClientRect();var p=svg.createSVGPoint();p.x=e.clientX;p.y=e.clientY;var m=svg.getScreenCTM();if(!m){return;}var q=p.matrixTransform(m.inverse());zoom(e.deltaY<0?0.85:1/0.85,q.x,q.y);},{passive:false});
var dragging=null;
svg.addEventListener("mousedown",function(e){dragging={x:e.clientX,y:e.clientY,v:view.slice()};svg.classList.add("drag");});
window.addEventListener("mousemove",function(e){if(!dragging){return;}var r=svg.getBoundingClientRect();var sx=dragging.v[2]/r.width,sy=dragging.v[3]/r.height,s=Math.max(sx,sy);view=[dragging.v[0]-(e.clientX-dragging.x)*s,dragging.v[1]-(e.clientY-dragging.y)*s,dragging.v[2],dragging.v[3]];setView();});
window.addEventListener("mouseup",function(){dragging=null;svg.classList.remove("drag");});
document.getElementById("zin").onclick=function(){zoom(0.8);};
document.getElementById("zout").onclick=function(){zoom(1.25);};
document.getElementById("reset").onclick=function(){view=base.slice();setView();};
var lb=document.getElementById("layout");lb.onclick=function(){circular=!circular;lb.textContent=circular?"Rectangular layout":"Circular layout";draw();};
var sel=document.getElementById("support");
D.supports.forEach(function(s,i){var o=document.createElement("option");o.value=i;o.textContent=s;sel.appendChild(o);});
var none=document.createElement("option");none.value=-1;none.textContent="none";sel.appendChild(none);
sel.value=supportIndex;sel.onchange=function(){supportIndex=parseInt(sel.value,10);draw();legend();};
function applySearch(){var n=0;var items=svg.querySelectorAll(".leaf");for(var i=0;i<items.length;i++){var on=query.length>0&&items[i].getAttribute("data-name").indexOf(query)>=0;items[i].classList.toggle("hit",on);if(on){n++;}}
 document.getElementById("hits").textContent=query.length?(n+(n===1?" match":" matches")):"";}
document.getElementById("search").addEventListener("input",function(e){query=e.target.value.trim().toLowerCase();applySearch();});
function bar(list){return "linear-gradient(to right,"+list.join(",")+")";}
function legend(){var f=document.getElementById("legend");f.innerHTML="";
 function add(html){var s=document.createElement("span");s.innerHTML=html;f.appendChild(s);}
 add("Halo value: 0<span class=\"bar\" style=\"background:"+bar(D.haloScale)+"\"></span>1 ");
 if(supportIndex>=0){add(" &middot; "+D.supports[supportIndex]+" support (branch colour and width): 0<span class=\"bar\" style=\"background:"+bar(D.supportScale)+"\"></span>1 ");}
 var gs=Object.keys(D.groupColours);if(gs.length){var t=document.createElement("span");t.textContent=" · Groups ("+D.groupSource+"):";f.appendChild(t);gs.forEach(function(gname){var s=document.createElement("span");var sw=document.createElement("span");sw.className="sw";sw.style.background=D.groupColours[gname];s.appendChild(sw);s.appendChild(document.createTextNode(gname));f.appendChild(s);});}
 var u=document.createElement("div");u.textContent="Unrooted tree, drawn rooted at the midpoint of its longest path; branch lengths are not drawn. Wheel or buttons to zoom, drag to pan, hover for values.";f.appendChild(u);}
draw();legend();
})();
"###;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_strings_cannot_close_the_script() {
        assert_eq!(js("a</script>\"b"), "\"a\\u003c/script\\u003e\\\"b\"");
        assert_eq!(js("tab\tline\n"), "\"tab\\tline\\n\"");
    }

    #[test]
    fn page_is_self_contained_and_carries_the_data() {
        let l = TreeLayout::from_newick("((A,B),(C,D),(E,F));").unwrap();
        let all = l.taxa();
        let mut s1 = EdgeSupport {
            label: "S1".into(),
            values: BTreeMap::new(),
        };
        s1.values.insert(
            crate::tree::canonical_split(&["A".to_string(), "B".to_string()], &all),
            0.75,
        );
        let halo: BTreeMap<String, Option<f64>> =
            all.iter().map(|t| (t.clone(), Some(0.5))).collect();
        let groups = Groups {
            source: "automatic".into(),
            of: [("A".to_string(), "G<1>".to_string())]
                .into_iter()
                .collect(),
        };
        let page = interactive_html(
            &l,
            &halo,
            &[s1],
            Some(&groups),
            &["Fish <mtDNA>".into(), "6 taxa".into()],
        );
        assert!(page.starts_with("<!DOCTYPE html>"));
        assert!(page.contains("<title>Fish &lt;mtDNA&gt;</title>"));
        // No external resources.
        for bad in ["src=\"http", "href=\"http", "<link", "@import", "url("] {
            assert!(!page.contains(bad), "{bad}");
        }
        assert!(page.contains("\"supports\":[\"S1\"]"));
        assert!(page.contains("0.7500"));
        assert!(page.contains("G\\u003c1\\u003e"));
        // The data block parses as JSON in principle: balanced braces.
        let start = page.find("type=\"application/json\">").unwrap();
        let block = &page[start..];
        let end = block.find("</script>").unwrap();
        let json = &block[24..end];
        assert_eq!(json.matches('{').count(), json.matches('}').count());
    }
}
