FROM python:3.10.7-slim-buster

RUN apt-get update
RUN apt-get install sshpass -y
RUN pip install ansible 

ENV ANSIBLE_CONFIG=/home/ansible.cfg

WORKDIR /home

CMD ["/bin/bash"]

# docker build -t python-slim -f Ansible/Dockerfile .
# docker run --rm -it -v ${pwd}\Ansible:/home/ python-slim
