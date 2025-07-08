# Python code with patterns to detect and fix

def process_data(name, age, city):
    # Old-style string formatting (should use f-strings)
    message = "Hello {}, you are {} years old and live in {}".format(name, age, city)
    print(message)

    # Another format usage
    log_message = "Processing user: {}".format(name)

    # Good examples (already using f-strings)
    good_message = f"Hello {name}, you are {age} years old"

    return message

class UserManager:
    def __init__(self):
        self.users = []

    def add_user(self, user_data):
        # String formatting to be converted
        status = "Adding user: {}".format(user_data.get('name', 'Unknown'))
        print(status)

        self.users.append(user_data)

    def get_user_info(self, user_id):
        user = self.find_user(user_id)
        if user:
            # Multiple format calls
            info = "User {} (ID: {}) - Email: {}".format(
                user['name'],
                user['id'],
                user['email']
            )
            return info
        return None

    def find_user(self, user_id):
        for user in self.users:
            if user['id'] == user_id:
                return user
        return None
