import re, sys
lines = open(sys.argv[1], encoding='latin-1').read().split('\n')
def dec(s):
    s = re.sub(r'\\(\d{3})', lambda m: {'050':'(', '051':')', '034':'fi', '033':'ff', '036':'ffi', '025':'-', '024':'<=', '025':'>=', '016':'delta', '034':'fi', '032':'rho', '020':'zeta', '021':'"', '013':'alpha', '014':'beta', '000':'-', '134':'\\', '022':'theta', '026':'mu', '010':'Phi', '003':'*', '004':'#', '015':'gamma', '031':'pi', '017':'eps', '035':'fl', '037':'ffl'}.get(m.group(1), '?'+m.group(1)), s)
    return s
for spec in sys.argv[2:]:
    a, b, w = (int(x) for x in spec.split(':'))
    for i in range(a-1, b):
        print(i+1, dec(lines[i])[:w])
