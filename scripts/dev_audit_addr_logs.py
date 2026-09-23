# Audit: positional addr-ish args in tracing macros not wrapped in log_*.
import re, glob

LIT = re.compile(r'"(?:\\.|[^"\\])*"')
ADDR_VARS = r'(remote_addr|peer_addr|target_addr|server_addr|dial_addr|remote_str|client_address|node_addr|src_ip|buddy_addr|contact_addr|from_addr)'

for path in glob.glob('crates/**/*.rs', recursive=True):
    text = open(path, encoding='utf-8').read()
    for m in re.finditer(r'(?<!\w)(?:tracing::)?(?:info|warn|error|debug|trace)!\s*\(', text):
        depth = 0
        j = text.index('(', m.start())
        instr = esc = False
        while j < len(text):
            c = text[j]
            if instr:
                if esc:
                    esc = False
                elif c == '\\':
                    esc = True
                elif c == '"':
                    instr = False
            else:
                if c == '"':
                    instr = True
                elif c == '(':
                    depth += 1
                elif c == ')':
                    depth -= 1
                    if depth == 0:
                        break
            j += 1
        call = text[m.start():j]
        args = LIT.sub('""', call)
        for vm in re.finditer(r'(?<![\w.])' + ADDR_VARS + r'(?!\s*[.(:])(\s*[,\)])', args):
            # Skip vars already wrapped: `log_*(<var>)`.
            prefix = args[max(0, vm.start() - 40):vm.start()]
            if re.search(r'log_\w+\(\s*[&*\s]*$', prefix):
                continue
            line = text[:m.start()].count('\n') + 1
            print(f'{path}:{line}: positional {vm.group(1)}')
