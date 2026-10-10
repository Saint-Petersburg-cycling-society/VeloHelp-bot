#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-only
"""Prepare ODbL offline addresses/streets from a public city PBF; no user points."""
from pathlib import Path
import argparse, hashlib, json
import osmium

class AddressData(osmium.SimpleHandler):
    def __init__(self):
        super().__init__()
        self.houses = set()
        self.roads = []
    def add_address(self, tags, points):
        street = tags.get('addr:street') or tags.get('addr:place')
        number = tags.get('addr:housenumber')
        if street and number and points:
            lon = (min(p[0] for p in points) + max(p[0] for p in points))/2
            lat = (min(p[1] for p in points) + max(p[1] for p in points))/2
            self.houses.add((round(lon,7),round(lat,7),street,number))
    def node(self, node):
        if node.location.valid():
            self.add_address(node.tags, [(node.location.lon,node.location.lat)])
    def way(self, way):
        if way.tags.get('highway') and way.tags.get('name'):
            points = [[round(n.lon,7),round(n.lat,7)] for n in way.nodes if n.location.valid()]
            if len(points) >= 2:
                self.roads.append([way.tags['name'],points])
    def area(self, area):
        if (area.tags.get('addr:street') or area.tags.get('addr:place')) and area.tags.get('addr:housenumber'):
            points = [(n.lon,n.lat) for ring in area.outer_rings() for n in ring if n.location.valid()]
            self.add_address(area.tags, points)

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--pbf',type=Path,required=True)
    args = parser.parse_args()
    data = AddressData()
    data.apply_file(args.pbf, locations=True, idx='flex_mem')
    assert data.houses and data.roads
    header = osmium.io.Reader(str(args.pbf)).header()
    output = {'license':'ODbL-1.0','attribution':'© OpenStreetMap contributors','source':'https://download.openstreetmap.fr/extracts/russia/northwestern_federal_district/saint_petersburg.osm.pbf','osm_timestamp':header.get('osmosis_replication_timestamp'), 'pbf_sha256':hashlib.sha256(args.pbf.read_bytes()).hexdigest(), 'houses':sorted(data.houses), 'roads':sorted(data.roads)}
    target = Path(__file__).with_name('spb-addresses.json')
    target.write_text(json.dumps(output,ensure_ascii=False,separators=(',',':'))+'\n')
    print(f'Prepared {len(data.houses)} address points and {len(data.roads)} named road geometries.')

if __name__ == '__main__':
    main()
