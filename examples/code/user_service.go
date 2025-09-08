package main

import (
	"fmt"
	"net/http"
)

// Constants for the application
const (
	ServerPort = ":8080"
	MaxRetries = 3
)

// Global variable
var globalCounter int

// User represents a user in the system
type User struct {
	ID    int    `json:"id"`
	Name  string `json:"name"`
	Email string `json:"email"`
}

// UserService interface defines user operations
type UserService interface {
	GetUser(id int) (*User, error)
	CreateUser(user User) error
	DeleteUser(id int) error
}

// InMemoryUserService implements UserService
type InMemoryUserService struct {
	users map[int]User
}

// NewInMemoryUserService creates a new in-memory user service
func NewInMemoryUserService() *InMemoryUserService {
	return &InMemoryUserService{
		users: make(map[int]User),
	}
}

// GetUser retrieves a user by ID
func (s *InMemoryUserService) GetUser(id int) (*User, error) {
	user, exists := s.users[id]
	if !exists {
		return nil, fmt.Errorf("user with ID %d not found", id)
	}
	return &user, nil
}

// CreateUser adds a new user to the service
func (s *InMemoryUserService) CreateUser(user User) error {
	s.users[user.ID] = user
	return nil
}

// DeleteUser removes a user from the service
func (s *InMemoryUserService) DeleteUser(id int) error {
	delete(s.users, id)
	return nil
}

// Helper function to format user
func formatUser(user *User) string {
	return fmt.Sprintf("User{ID: %d, Name: %s, Email: %s}", 
		user.ID, user.Name, user.Email)
}

// HTTP handler function
func handleUsers(w http.ResponseWriter, r *http.Request) {
	fmt.Fprintf(w, "Users endpoint")
}

func main() {
	service := NewInMemoryUserService()
	
	// Create a test user
	testUser := User{
		ID:    1,
		Name:  "John Doe", 
		Email: "john@example.com",
	}
	
	err := service.CreateUser(testUser)
	if err != nil {
		fmt.Printf("Error creating user: %v\n", err)
		return
	}

	user, err := service.GetUser(1)
	if err != nil {
		fmt.Printf("Error getting user: %v\n", err)
		return
	}

	fmt.Println(formatUser(user))
	
	http.HandleFunc("/users", handleUsers)
	fmt.Printf("Server starting on %s\n", ServerPort)
	http.ListenAndServe(ServerPort, nil)
}