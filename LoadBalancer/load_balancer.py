import random


class LoadBalancer:
    def __init__(self):
        # do intialization if necessary
        self.servers = []
        self.ser_index = {}

    """
    @param: server_id: add a new server to the cluster
    @return: nothing
    """

    def add(self, server_id):
        # write your code here
        self.servers.append(server_id)
        self.ser_index[server_id] = len(self.servers) - 1

    """
    @param: server_id: server_id remove a bad server from the cluster
    @return: nothing
    """

    def remove(self, server_id):
        # write your code here
        swap_ser = self.servers[-1]
        rem_ser_index = self.ser_index[server_id]
        self.servers[rem_ser_index] = self.servers[-1]
        self.servers.pop()

        self.ser_index[swap_ser] = rem_ser_index
        del self.ser_index[server_id]
    """
    @return: pick a server in the cluster randomly with equal probability
    """

    def pick(self):
        # write your code here
        return self.servers[random.randint(0, len(self.servers) - 1)]
