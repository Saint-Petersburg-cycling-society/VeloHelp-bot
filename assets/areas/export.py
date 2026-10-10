#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-only
"""Prepare ODbL district rings from public OSM relations; never user coordinates."""
from pathlib import Path
import argparse, json, urllib.request

RELATIONS = dict(zip([
    'admiralteysky','vasileostrovsky','vyborgsky','kalininsky','kirovsky','kolpinsky',
    'krasnogvardeysky','krasnoselsky','kronshtadtsky','kurortny','moskovsky','nevsky',
    'petrogradsky','petrodvortsovy','primorsky','pushkinsky','frunzensky','tsentralny'
], [1114193,1114252,1114354,1114806,1114809,337424,1114895,363103,1115082,
    1115366,338636,368287,1114905,367375,1115367,338635,369514,1114902]))

def rings(parts):
    result=[]
    while parts:
        ring=parts.pop()
        while ring[-1]!=ring[0]:
            for index,part in enumerate(parts):
                if part[0]==ring[-1]: break
                if part[-1]==ring[-1]: part=list(reversed(part)); break
            else: raise ValueError('Unclosed boundary ring')
            ring.extend(part[1:]);parts.pop(index)
        assert len(ring)>=4
        result.append(ring)
    return result

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cache-dir',type=Path,required=True)
    args=parser.parse_args();args.cache_dir.mkdir(parents=True,exist_ok=True)
    districts=[]
    for area,osm_id in RELATIONS.items():
        cache=args.cache_dir/(str(osm_id)+'.json')
        if not cache.exists():
            request=urllib.request.Request(f'https://api.openstreetmap.org/api/0.6/relation/{osm_id}/full.json',headers={'User-Agent':'VeloHelp offline boundary preparation'})
            with urllib.request.urlopen(request,timeout=40) as response: raw=json.load(response)
            cache.write_text(json.dumps(raw,ensure_ascii=False))
        raw=json.loads(cache.read_text());elements=raw['elements']
        relation=next(e for e in elements if e['type']=='relation' and e['id']==osm_id)
        assert relation['tags']['admin_level']=='5'
        nodes={e['id']:[e['lon'],e['lat']] for e in elements if e['type']=='node'}
        ways={e['id']:e['nodes'] for e in elements if e['type']=='way'}
        geometry={}
        for role in ('outer','inner'):
            parts=[]
            for member in relation['members']:
                if member['role']==role:
                    assert member['type']=='way','Nested geometry needs explicit conversion'
                    parts.append(ways[member['ref']].copy())
            geometry[role]=[[nodes[node] for node in ring] for ring in rings(parts)]
        assert geometry['outer']
        points=[p for ring in geometry['outer'] for p in ring]
        geometry['bbox']=[min(p[0] for p in points),min(p[1] for p in points),max(p[0] for p in points),max(p[1] for p in points)]
        districts.append({'id':'spb-'+area,'name':relation['tags']['name'],'osm_relation_id':osm_id,'osm_version':relation['version'],**geometry})
    output={'license':'ODbL-1.0','attribution':'© OpenStreetMap contributors','source':'https://www.openstreetmap.org/copyright','districts':sorted(districts,key=lambda d:d['id'])}
    path=Path(__file__).with_name('spb-districts.json')
    path.write_text(json.dumps(output,ensure_ascii=False,separators=(',',':'))+'\n')
    print(f'{len(districts)} districts, {sum(len(r) for d in districts for r in d["outer"])} outer vertices; closed rings verified')

if __name__=='__main__': main()
