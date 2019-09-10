#!/usr/bin/env python3
import pandas as pd

def parse_block(lines, run_format, run_date):
    # Future reference: output benchmark results in a format
    # that doesn't require extra code to read
    return {
        'run_date': run_date,
        'run_format' : run_format,
        'protocol' : lines[0].split(' total')[0],
        'total_secs' : int(lines[0].split('=')[1][:-2]),
        'serialize_50_nanos' : int(lines[2].split('=')[1][:-3]),
        'serialize_99_nanos' : int(lines[3].split('=')[1][:-3]),
        'serialize_999_nanos' : int(lines[4].split('=')[1][:-3]),
        'deserialize_50_nanos' : int(lines[5].split('=')[1][:-3]),
        'deserialize_99_nanos' : int(lines[6].split('=')[1][:-3]),
        'deserialize_999_nanos' : int(lines[7].split('=')[1][:-3]),
        'serialize_total_nanos' : int(lines[8].split('=')[1][:-3]),
        'deserialize_total_nanos' : int(lines[9].split('=')[1][:-3]),
    }


def main(filename: str, run_format: str):
    records = []
    run_count = 10
    
    with open(filename, 'r') as handle:
        lines = handle.readlines()

    num_blocks = 4
    num_dates = 4
    block_len = 12

    current_line = 0
    for i in range(run_count):
        for d in range(num_dates):
            run_date = lines[current_line].split('_')[2]
            current_line += 1

            for block in range(num_blocks):
                lower_block = current_line
                upper_block = current_line + block_len

                rec = parse_block(lines[lower_block:upper_block], run_format, run_date)
                records.append(rec)
                current_line += block_len

    return records


if __name__ == '__main__':
    all_records = []

    runs = [
        ('shootout_normal.txt', ''),
        ('shootout_taskset.txt', 'taskset'),
        ('shootout_nice.txt', 'taskset + nice')
    ]
    for fname, run_format in runs:
        for record in main(fname, run_format):
            all_records.append(record)

    pd.DataFrame.from_records(all_records).to_csv('shootout.csv', index=False)
