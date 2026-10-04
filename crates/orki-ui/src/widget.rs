use iced::widget::shader::Shader;

use crate::shader::AuroraShader;

pub type AuroraWidget<Message> = Shader<Message, AuroraShader>;

pub fn aurora<Message: 'static>(progress: f32) -> AuroraWidget<Message> {
    Shader::new(AuroraShader::new(progress))
}
