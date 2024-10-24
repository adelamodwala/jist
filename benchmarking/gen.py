import json
import random
import string
import sys

count = 10000000
if(len(sys.argv)>1):
    count = int(sys.argv[1])

def randomString(len):
    """Generate a random string of fixed length """
    letters = string.ascii_letters
    return ''.join(random.choice(letters) for i in range(len))
def randomVal():
    words=["true", "false", "null"]
    return random.choice(words)

def randomArray(len):
    return [random.randint(0, 100) for x in range(len)]

answer = []
for _ in range(count):
    my_dict = {    'foo': random.randint(0, 100),    'bar': {'baz': randomString(random.randint(0, 100)),       'poo': randomVal(), 'bizbizbiz': randomString(random.randint(20, 30)), 'bouou':randomArray(random.randint(0, 10)) }}
    answer.append(my_dict)
print(json.dumps(answer, sort_keys=True, indent=4))