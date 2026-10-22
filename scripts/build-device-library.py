"""Converte dados licenciados KiCad/Espressif para o catálogo físico do ProtoCraft.
Execute na raiz: python scripts/build-device-library.py. Downloads ficam em artifacts/.
Somente dados; nenhum código de modelos ou binário de terceiros é executado.
"""
import argparse, concurrent.futures, hashlib, json, math, re, tarfile, urllib.request
from pathlib import Path
from decimal import Decimal
BASE=Path(__file__).resolve().parent.parent
CACHE=BASE/'artifacts/device-sources'
DEST=BASE/'assets/library'
LICENSE='CC-BY-SA-4.0 WITH KiCad-library-exception'
KISYM='7800d91437ce44e2ed0928f2ad31a287457b8a68'
ESP='dd76561812ab300351234ba6e0ec1295641796f0'
def download(url):
    req=urllib.request.Request(url,headers={'User-Agent':'ProtoCraft-library-builder'})
    with urllib.request.urlopen(req,timeout=120) as r:return r.read()
def parse(text):
    tokens=re.findall(r'"(?:\\.|[^"\\])*"|[()]|[^\s()]+',text)
    stack=[];root=None
    for t in tokens:
        if t=='(':
            node=[]
            if stack:stack[-1].append(node)
            stack.append(node)
        elif t==')':
            node=stack.pop()
            if not stack:root=node
        else:
            if t.startswith('"'):t=json.loads(t)
            stack[-1].append(t)
    if stack or root is None:raise ValueError('S-expression incompleta')
    return root

def children(node,key):return [v for v in node if isinstance(v,list) and v and v[0]==key]
def child(node,key,default=None):return next(iter(children(node,key)),default)
def scalar(node,key,default=''):return (child(node,key) or [key,default])[1]
def nm(x):return int(Decimal(str(x))*1000000)
def point(values):return {'x':nm(values[0]),'y':nm(values[1])}
def source(name,sha,path):
    project='Espressif KiCad Libraries' if name=='espressif' else 'KiCad Libraries'
    url=('https://github.com/espressif/kicad-libraries/blob/'+sha+'/' if name=='espressif' else 'https://gitlab.com/kicad/libraries/'+name+'/-/blob/'+sha+'/')+path
    return dict(project=project,revision=sha,url=url,license=LICENSE)

def acquire():
    CACHE.mkdir(parents=True,exist_ok=True);DEST.mkdir(parents=True,exist_ok=True)
    for name,sha,url in [('kicad-symbols',KISYM,'https://gitlab.com/api/v4/projects/kicad%2Flibraries%2Fkicad-symbols/repository/archive.tar.gz?sha='+KISYM),('espressif',ESP,'https://codeload.github.com/espressif/kicad-libraries/tar.gz/'+ESP)]:
        target=CACHE/name
        archive=CACHE/('symbols.tar.gz' if name=='kicad-symbols' else 'espressif.tar.gz')
        if not archive.exists():archive.write_bytes(download(url))
        with tarfile.open(archive,'r:gz') as tar:
            for m in tar:
                parts=Path(m.name).parts[1:]
                if not m.isfile() or not parts:continue
                keep=parts==('LICENSE.md',) or (name=='kicad-symbols' and (parts[0].startswith('MCU_ST_STM32') or parts[0] in ('MCU_Espressif.kicad_symdir','RF_Module.kicad_symdir')) and m.name.endswith('.kicad_sym')) or (name=='espressif' and parts[0] in ('symbols','footprints'))
                if keep:
                    path=target.joinpath(*parts);path.parent.mkdir(parents=True,exist_ok=True)
                    if not path.exists():path.write_bytes(tar.extractfile(m).read())
    revfile=CACHE/'footprints-commit.txt'
    if revfile.exists(): fp_sha=revfile.read_text().strip()
    else:
        fp_sha=json.loads(download('https://gitlab.com/api/v4/projects/kicad%2Flibraries%2Fkicad-footprints/repository/commits/10.0.6'))['id'];revfile.write_text(fp_sha)
    return fp_sha

def load_symbols():
    groups=[]
    for directory in sorted((CACHE/'kicad-symbols').glob('MCU_ST_STM32*.kicad_symdir')):
        nodes={}
        for path in directory.glob('*.kicad_sym'):
            root=parse(path.read_text(encoding='utf-8'))
            for s in children(root,'symbol'):nodes[s[1]]=(s,path.relative_to(CACHE/'kicad-symbols').as_posix())
        groups.append((directory.name.split('.')[0].replace('MCU_ST_',''),'STMicroelectronics',nodes,'kicad-symbols',KISYM))
    # Biblioteca oficial Espressif inclui SoCs, módulos e DevKits atuais.
    nodes={}
    for path in (CACHE/'espressif/symbols').glob('*.kicad_sym'):
        for s in children(parse(path.read_text(encoding='utf-8')),'symbol'):nodes[s[1]]=(s,path.relative_to(CACHE/'espressif').as_posix())
    groups.append(('ESP32','Espressif',nodes,'espressif',ESP))
    # Complementa variantes históricas dos módulos, com a mesma licença de dados.
    for dirname in ('MCU_Espressif.kicad_symdir','RF_Module.kicad_symdir'):
        nodes={}
        for path in (CACHE/'kicad-symbols'/dirname).glob('*.kicad_sym'):
            for s in children(parse(path.read_text(encoding='utf-8')),'symbol'):nodes[s[1]]=(s,path.relative_to(CACHE/'kicad-symbols').as_posix())
        groups.append(('ESP32','Espressif',nodes,'kicad-symbols',KISYM))
    return groups

def resolve(name,nodes,seen=None):
    seen=set() if seen is None else seen
    if name in seen:raise ValueError('Ciclo extends: '+name)
    seen.add(name);node,path=nodes[name]
    props={p[1]:p[2] for p in children(node,'property')}
    pins=[]
    parent=scalar(node,'extends')
    if parent:
        base,basepins=resolve(parent,nodes,seen);base.update(props);props=base;pins=basepins
    local=[]
    for unit in children(node,'symbol'):
        for pin in children(unit,'pin'):
            at=child(pin,'at');number=scalar(pin,'number');pname=scalar(pin,'name')
            local.append((number,pname,pin[1],Decimal(at[1]),-Decimal(at[2])))
    if local:pins=local
    return props,pins

def footprint(name,fp_sha):
    lib,filename=name.split(':',1)
    official=CACHE/'espressif/footprints'/f'{lib.removeprefix('PCM_')}.pretty'/f'{filename}.kicad_mod'
    if official.exists():path=official;origin=source('espressif',ESP,official.relative_to(CACHE/'espressif').as_posix())
    else:
        rel=f'{lib}.pretty/{filename}.kicad_mod';path=CACHE/'kicad-footprints'/rel
        if not path.exists():
            path.parent.mkdir(parents=True,exist_ok=True)
            from urllib.parse import quote
            path.write_bytes(download('https://gitlab.com/api/v4/projects/kicad%2Flibraries%2Fkicad-footprints/repository/files/'+quote(rel,safe='')+'/raw?ref='+fp_sha))
        origin=source('kicad-footprints',fp_sha,rel)
    root=parse(path.read_text(encoding='utf-8'));pads=[];lines=[];keepouts=[]
    fab=[]
    def addline(a,b,layer,width):
        lines.append(dict(start=a,end=b,width_nm=width,layer=layer))
        if layer=='F.Fab':fab.extend([a,b])
    for node in root:
        if not isinstance(node,list) or not node:continue
        tag=node[0]
        if tag=='pad':
            number,tech,shape=node[1:4];at=child(node,'at');size=child(node,'size');drill=child(node,'drill');layers=child(node,'layers')[1:]
            if scalar(node,'property')=='pad_prop_heatsink':pass
            polygons=[]
            if shape=='custom':
                primitives=child(node,'primitives',[])
                for poly in children(primitives,'gr_poly'):
                    if scalar(poly,'fill')!='yes':raise ValueError('Primitiva custom não preenchida')
                    polygons.append([point(p[1:]) for p in children(child(poly,'pts',[]),'xy')])
                if not polygons:raise ValueError('Pad custom sem polígonos suportados')
            drill_size=None
            if drill:
                values=[x for x in drill[1:] if isinstance(x,str) and x!='oval']
                if len(values)==1:values*=2
                drill_size=point(values[:2])
            if shape not in ('rect','roundrect','circle','oval','custom'):raise ValueError('Formato de pad '+shape)
            pads.append(dict(number=number,position=point(at[1:3]),size=point(size[1:3]),rotation_mdeg=int(Decimal(at[3])*1000) if len(at)>3 else 0,shape={'rect':'Rect','roundrect':'RoundRect','circle':'Circle','oval':'Oval','custom':'Custom'}[shape],technology={'smd':'Smd','thru_hole':'ThroughHole','np_thru_hole':'NonPlated'}[tech],drill=drill_size,layers=layers,roundrect_ratio_ppm=int(Decimal(scalar(node,'roundrect_rratio','0'))*1000000),polygons=polygons,mask_margin_nm=nm(scalar(node,'solder_mask_margin',scalar(root,'solder_mask_margin','0'))),paste_margin_nm=nm(scalar(node,'solder_paste_margin',scalar(root,'solder_paste_margin','0'))),paste_ratio_ppm=int(Decimal(scalar(node,'solder_paste_margin_ratio',scalar(root,'solder_paste_margin_ratio','0')))*1000000)))
        elif tag in ('fp_line','fp_rect','fp_poly','fp_circle','fp_arc'):
            layer=scalar(node,'layer');stroke=child(node,'stroke',[]);width=nm(scalar(stroke,'width','0.1'))
            if tag in ('fp_line','fp_rect'):
                a=point(child(node,'start')[1:]);b=point(child(node,'end')[1:])
                if tag=='fp_line':addline(a,b,layer,width)
                else:
                    corners=[a,dict(x=b['x'],y=a['y']),b,dict(x=a['x'],y=b['y'])]
                    for a,b in zip(corners,corners[1:]+corners[:1]):addline(a,b,layer,width)
            elif tag=='fp_poly':
                corners=[point(p[1:]) for p in children(child(node,'pts',[]),'xy')]
                for a,b in zip(corners,corners[1:]+corners[:1]):addline(a,b,layer,width)
            elif tag=='fp_circle':
                c=point(child(node,'center')[1:]);e=point(child(node,'end')[1:]);r=math.hypot(e['x']-c['x'],e['y']-c['y'])
                corners=[dict(x=round(c['x']+r*math.cos(i*math.tau/64)),y=round(c['y']+r*math.sin(i*math.tau/64))) for i in range(64)]
                for a,b in zip(corners,corners[1:]+corners[:1]):addline(a,b,layer,width)
            elif tag=='fp_arc':
                # Curvas de silk/fab são aproximadas somente no desenho; pads permanecem exatos.
                if layer.endswith('.Cu'):raise ValueError('Arco de cobre não suportado')
                for key in ('start','mid','end'):
                    p=child(node,key)
                    if p and layer=='F.Fab':fab.append(point(p[1:]))
        elif tag=='zone' and child(node,'keepout'):
            ko=child(node,'keepout');pol=child(node,'polygon',[])
            points=[point(p[1:]) for p in children(child(pol,'pts',[]),'xy')]
            keepouts.append(dict(polygon=points,tracks=scalar(ko,'tracks')=='not_allowed',vias=scalar(ko,'vias')=='not_allowed',pads=scalar(ko,'pads')=='not_allowed',copper=scalar(ko,'copperpour')=='not_allowed'))
    body_layer='F.Fab'
    if not fab:
        fab=[p for l in lines if l['layer']=='Eco2.User' for p in (l['start'],l['end'])];body_layer='Eco2.User'
    if not fab:raise ValueError('Sem contorno mecânico F.Fab/Eco2.User para dimensões do corpo')
    pads=list({json.dumps(p,sort_keys=True):p for p in pads}.values())
    return dict(id=name,description=scalar(root,'descr'),body_layer=body_layer,body_bounds={'min':{'x':min(p['x'] for p in fab),'y':min(p['y'] for p in fab)},'max':{'x':max(p['x'] for p in fab),'y':max(p['y'] for p in fab)}},pads=pads,lines=lines,keepouts=keepouts,source=origin)

def main():
    fp_sha=acquire();devices=[];symbols={};extra={'Resistor_SMD:R_0603_1608Metric','Resistor_SMD:R_0805_2012Metric','Capacitor_SMD:C_0603_1608Metric','Capacitor_SMD:C_0805_2012Metric','Package_DIP:DIP-8_W7.62mm'};requested=set(extra);skipped=[]
    for family,maker,nodes,repo,sha in load_symbols():
        for name,(node,path) in sorted(nodes.items()):
            if maker=='STMicroelectronics' and '_' in name:continue
            if maker=='Espressif' and not (name.startswith('ESP32') or name.startswith('ESP868')):continue
            try:
                props,pins=resolve(name,nodes);fp=props.get('Footprint','')
                if not fp or not pins:raise ValueError('Sem footprint ou pinos')
                # KiCad 10 permite grupos explícitos como [1,15,38,39]; cada número
                # torna-se um pino físico, preservando o nome/tipo e a posição compartilhada.
                expanded=[]
                for number,pname,typ,x,y in pins:
                    if number.startswith('['):
                        if not re.fullmatch(r'\[[A-Za-z0-9]+(?:,[A-Za-z0-9]+)*\]',number):raise ValueError('Grupo de números não suportado '+number)
                        expanded.extend((n,pname,typ,x,y) for n in number[1:-1].split(','))
                    else:expanded.append((number,pname,typ,x,y))
                pins=expanded
                unique={}
                for number,pname,typ,x,y in pins:
                    if number in unique:raise ValueError('Número de pino repetido '+number)
                    unique[number]=(number,pname,typ,x,y)
                pins=list(unique.values());step=Decimal('1.27')
                if any(x%step or y%step for _,_,_,x,y in pins):step=Decimal('0.635')
                if any(x%step or y%step for _,_,_,x,y in pins):raise ValueError('Grade fracionária não suportada')
                converted=[dict(number=n,name=pn,electrical_type=t,offset=dict(x=int(x/step),y=int(y/step)),pads=[n]) for n,pn,t,x,y in pins]
                key=hashlib.sha256(json.dumps(converted,sort_keys=True).encode()).hexdigest()[:20];symbols[key]=converted
                form='Chip'
                if maker=='Espressif' and any(v in name.upper() for v in ('WROOM','WROVER','MINI','PICO-MINI','SOLO')):form='Module'
                if maker=='Espressif' and any(v in name.upper() for v in ('DEVKIT','DEVBOARD')):form='Board'
                record=dict(id=name,manufacturer=maker,family=family,form=form,description=props.get('Description',''),datasheet=props.get('Datasheet',''),symbol=key,footprint=fp,source=source(repo,sha,path))
                variants={'ESP32-C6-MINI-1/U':['ESP32-C6-MINI-1','ESP32-C6-MINI-1U'],'ESP8684-WROOM-02C/U':['ESP8684-WROOM-02C','ESP8684-WROOM-02UC']}.get(name)
                if variants:
                    for variant in variants:
                        row=record.copy();row['id']=variant;row['footprint']=fp.split(':')[0]+':'+variant;devices.append(row);requested.add(row['footprint'])
                    skipped.append(dict(id=name,reason='Grupo separado em variantes físicas explícitas',variants=variants))
                elif '/' in name:skipped.append(dict(id=name,reason='Identificador com variantes físicas ambíguas'))
                else:devices.append(record);requested.add(fp)
            except Exception as e:skipped.append(dict(id=name,reason=str(e)))
    footprints={}
    def fetch(name):
        try:return name,footprint(name,fp_sha),None
        except Exception as e:return name,None,str(e)
    with concurrent.futures.ThreadPoolExecutor(max_workers=8) as pool:
        for name,value,error in pool.map(fetch,sorted(requested)):
            if value:footprints[name]=value
            else:print('Footprint indisponível',name,error,flush=True)
    accepted=[]
    for d in devices:
        f=footprints.get(d['footprint'])
        if not f:skipped.append(dict(id=d['id'],reason='Footprint indisponível'));continue
        mapped={p['number'] for p in symbols[d['symbol']]}
        physical={p['number'] for p in f['pads'] if p['number'] and p['technology']!='NonPlated'}
        if mapped!=physical:
            skipped.append(dict(id=d['id'],reason='Pinos e pads diferem',missing_pads=sorted(mapped-physical),extra_pads=sorted(physical-mapped)));continue
        accepted.append(d)
    accepted=list({d['id']:d for d in reversed(accepted)}.values());accepted.sort(key=lambda d:d['id'])
    used={d['symbol'] for d in accepted};symbols={k:v for k,v in symbols.items() if k in used}
    used={d['footprint'] for d in accepted}|extra;footprints={k:v for k,v in footprints.items() if k in used}
    catalog=dict(schema_version=1,devices=accepted,symbols=symbols,footprints=footprints)
    data=json.dumps(catalog,ensure_ascii=False,separators=(',',':')).encode('utf-8');(DEST/'catalog.json').write_bytes(data)
    report=dict(kicad_symbols_commit=KISYM,kicad_footprints_commit=fp_sha,espressif_commit=ESP,devices=len(accepted),symbols=len(symbols),footprints=len(footprints),families={f:sum(d['family']==f for d in accepted) for f in sorted({d['family'] for d in accepted})},forms={f:sum(d['form']==f for d in accepted) for f in sorted({d['form'] for d in accepted})},max_pins=max(len(symbols[d['symbol']]) for d in accepted),excluded=[r for r in skipped if r['id'] not in {d['id'] for d in accepted}],source_rejections=skipped,sha256=hashlib.sha256(data).hexdigest())
    (DEST/'coverage.json').write_text(json.dumps(report,ensure_ascii=False,indent=2),encoding='utf-8')
    for name in ('kicad-symbols','espressif'):(DEST/(name+'-LICENSE.md')).write_bytes((CACHE/name/'LICENSE.md').read_bytes())
    print(json.dumps({k:v for k,v in report.items() if k!='excluded'},indent=2));print('Excluídos',len(skipped),'bytes',len(data))
if __name__=='__main__':main()
