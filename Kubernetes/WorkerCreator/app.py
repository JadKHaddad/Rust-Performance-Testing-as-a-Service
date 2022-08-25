import os
import sys

if __name__ == '__main__':
    worker_nr = 0
    if len(sys.argv) > 1:
        worker_nr = int(sys.argv[1])
    dir_path = os.path.dirname(__file__)
    template_path = os.path.join(dir_path, 'template.yaml')
    deployment_path = os.path.join(dir_path, f'worker-{worker_nr}.yaml')
    with open(template_path, 'r', encoding='utf-8') as file:
        template = file.read()
        template = template.format(worker_nr=worker_nr)
        with open(deployment_path, 'w', encoding='utf-8') as file:
            file.write(template)
